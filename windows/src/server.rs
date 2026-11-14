use crate::storage;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Notify, Semaphore},
};
use tokio_util::sync::CancellationToken;

pub type Ui = Arc<dyn Fn(String) + Send + Sync>;
#[derive(Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub text: Option<String>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}
pub struct Pending {
    pub name: String,
    pub answer: tokio::sync::oneshot::Sender<bool>,
}
pub struct App {
    pub invitation: String,
    pub fingerprint: String,
    pub cert: Vec<u8>,
    pub key: Vec<u8>,
    pub session: Mutex<Option<String>>,
    pub pending: Mutex<Option<Pending>>,
    pub queue: Mutex<VecDeque<Item>>,
    pub notify: Notify,
    pub ui: Ui,
    pub received: PathBuf,
    pub stage: PathBuf,
    pub cancel: Mutex<CancellationToken>,
    pub transfers: Arc<Semaphore>,
    pub pairing: Semaphore,
}
type ApiError = (StatusCode, String);
fn err(e: impl std::fmt::Display) -> ApiError {
    (StatusCode::BAD_REQUEST, e.to_string())
}
fn secret() -> String {
    let mut b = [0; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}
impl App {
    pub fn new(root: PathBuf, ui: Ui) -> anyhow::Result<Arc<Self>> {
        let received = root.join("Received");
        let stage = root.join("Staging");
        std::fs::create_dir_all(&received)?;
        std::fs::create_dir_all(&stage)?;
        // Staging is owned by this application. Never promote leftovers after a crash.
        for e in std::fs::read_dir(&stage)? {
            let p = e?.path();
            if p.extension().is_some_and(|s| s == "part") {
                std::fs::remove_file(p)?;
            }
        }
        let cert = rcgen::generate_simple_self_signed(vec!["localdrop.local".into()])?;
        let fingerprint = hex::encode(Sha256::digest(cert.cert.der().as_ref()));
        Ok(Arc::new(Self {
            invitation: secret(),
            fingerprint,
            cert: cert.cert.pem().into_bytes(),
            key: cert.key_pair.serialize_pem().into_bytes(),
            session: Mutex::new(None),
            pending: Mutex::new(None),
            queue: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            ui,
            received,
            stage,
            cancel: Mutex::new(CancellationToken::new()),
            transfers: Arc::new(Semaphore::new(1)),
            pairing: Semaphore::new(1),
        }))
    }
    pub fn cancel_all(&self) {
        let mut c = self.cancel.lock().unwrap();
        c.cancel();
        *c = CancellationToken::new();
        for item in self.queue.lock().unwrap().drain(..) {
            if let Some(path) = item.path {
                let _ = std::fs::remove_file(path);
            }
        }
        self.notify.notify_waiters();
        (self.ui)("Передачи отменены".into());
    }
    pub fn disconnect(&self) {
        self.answer(false);
        *self.session.lock().unwrap() = None;
        self.cancel_all();
    }
    pub fn answer(&self, yes: bool) {
        if let Some(p) = self.pending.lock().unwrap().take() {
            let _ = p.answer.send(yes);
        }
    }
    pub fn enqueue(&self, item: Item) -> anyhow::Result<()> {
        let mut q = self.queue.lock().unwrap();
        anyhow::ensure!(q.len() < 128, "Очередь заполнена");
        q.push_back(item);
        drop(q);
        self.notify.notify_one();
        Ok(())
    }
    pub async fn queue_file(self: Arc<Self>, path: PathBuf) -> anyhow::Result<()> {
        let cancel = self.cancel.lock().unwrap().clone();
        let (name, size, digest, part) = tokio::task::spawn_blocking({
            let app = self.clone();
            let cancel = cancel.clone();
            move || -> anyhow::Result<_> {
                let name = path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
                    .to_string_lossy()
                    .to_string();
                let (part, mut out) = storage::create_partial(&app.stage)?;
                let mut input = std::fs::File::open(&path)?;
                let mut h = Sha256::new();
                let mut b = [0; 65536];
                let mut size = 0;
                let total = input.metadata()?.len();
                let mut last = Instant::now();
                (app.ui)(format!("Подготовка: {name}"));
                loop {
                    anyhow::ensure!(!cancel.is_cancelled(), "Отменено");
                    let n = std::io::Read::read(&mut input, &mut b)?;
                    if n == 0 {
                        break;
                    }
                    std::io::Write::write_all(&mut out, &b[..n])?;
                    h.update(&b[..n]);
                    size += n as u64;
                    if last.elapsed() >= Duration::from_millis(250) {
                        (app.ui)(format!("Подготовка: {size} / {total} байт"));
                        last = Instant::now();
                    }
                }
                out.sync_all()?;
                drop(out);
                Ok((name, size, hex::encode(h.finalize()), part))
            }
        })
        .await??;
        let _epoch = self.cancel.lock().unwrap();
        anyhow::ensure!(!cancel.is_cancelled(), "Отменено");
        let item = Item {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            size,
            sha256: digest,
            text: None,
            path: Some(part.0.clone()),
        };
        self.enqueue(item)?;
        let _ = part.keep();
        (self.ui)(format!("В очереди: {size} байт"));
        Ok(())
    }
}
fn auth(app: &App, h: &HeaderMap) -> Result<(), ApiError> {
    let expected = app.session.lock().unwrap();
    let supplied = h
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .unwrap_or("");
    if expected
        .as_ref()
        .is_some_and(|s| bool::from(format!("Bearer {s}").as_bytes().ct_eq(supplied.as_bytes())))
    {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "Pair first".into()))
    }
}
#[derive(Deserialize)]
struct Pair {
    token: String,
    name: String,
}
async fn pair(
    State(a): State<Arc<App>>,
    Json(p): Json<Pair>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _permit = a
        .pairing
        .try_acquire()
        .map_err(|_| (StatusCode::CONFLICT, "Pairing already pending".into()))?;
    if !bool::from(a.invitation.as_bytes().ct_eq(p.token.as_bytes()))
        || a.session.lock().unwrap().is_some()
    {
        return Err((StatusCode::FORBIDDEN, "Invalid or already paired".into()));
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let name: String = p
        .name
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    *a.pending.lock().unwrap() = Some(Pending {
        name: name.clone(),
        answer: tx,
    });
    (a.ui)(format!(
        "Запрос сопряжения: {name}. Нажмите Разрешить или Отклонить."
    ));
    let accepted = tokio::time::timeout(Duration::from_secs(60), rx)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    a.pending.lock().unwrap().take();
    if !accepted {
        return Err((
            StatusCode::FORBIDDEN,
            "Pairing declined or timed out".into(),
        ));
    }
    let token = secret();
    *a.session.lock().unwrap() = Some(token.clone());
    (a.ui)(format!("Сопряжено: {name}"));
    Ok(Json(
        serde_json::json!({"session": token, "name": "LocalDrop Windows"}),
    ))
}
async fn next(State(a): State<Arc<App>>, h: HeaderMap) -> Result<Response, ApiError> {
    auth(&a, &h)?;
    let notified = a.notify.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();
    if let Some(item) = a.queue.lock().unwrap().front().cloned() {
        return Ok(Json(item).into_response());
    }
    let _ = tokio::time::timeout(Duration::from_secs(50), notified).await;
    auth(&a, &h)?;
    Ok(match a.queue.lock().unwrap().front().cloned() {
        Some(i) => Json(i).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}
async fn ack(
    State(a): State<Arc<App>>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    auth(&a, &h)?;
    let mut q = a.queue.lock().unwrap();
    if q.front().is_some_and(|x| x.id == id) {
        if let Some(i) = q.pop_front() {
            if let Some(p) = i.path {
                let _ = std::fs::remove_file(p);
            }
            (a.ui)(format!("Доставлено: {}", i.name));
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn download(
    State(a): State<Arc<App>>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    auth(&a, &h)?;
    let item = a
        .queue
        .lock()
        .unwrap()
        .iter()
        .find(|x| x.id == id)
        .cloned()
        .ok_or((StatusCode::NOT_FOUND, "Unknown transfer".into()))?;
    let f = tokio::fs::File::open(item.path.ok_or_else(|| err("Not a file"))?)
        .await
        .map_err(err)?;
    let permit = a
        .transfers
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::CONFLICT, "Busy".into()))?;
    let cancel = a.cancel.lock().unwrap().clone();
    let ui = a.ui.clone();
    let total = item.size;
    let stream = futures_util::stream::try_unfold(
        (f, permit, 0u64, Instant::now()),
        move |(mut f, permit, mut done, mut last)| {
            let cancel = cancel.clone();
            let ui = ui.clone();
            async move {
                let mut b = vec![0; 65536];
                let n = tokio::select! { _ = cancel.cancelled() => return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,"Cancelled")), r = f.read(&mut b) => r? };
                if n == 0 {
                    return Ok(None);
                }
                b.truncate(n);
                done += n as u64;
                if last.elapsed() > Duration::from_millis(250) || done == total {
                    ui(format!("Отправка: {done} / {total} байт"));
                    last = Instant::now();
                }
                Ok(Some((b, (f, permit, done, last))))
            }
        },
    );
    Ok((
        [
            ("content-length", item.size.to_string()),
            ("content-type", "application/octet-stream".into()),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}
#[derive(Deserialize)]
struct Upload {
    name: String,
    size: u64,
    sha256: String,
}
async fn upload(
    State(a): State<Arc<App>>,
    h: HeaderMap,
    Query(q): Query<Upload>,
    body: Body,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&a, &h)?;
    let result = receive_upload(a.clone(), q, body).await;
    if let Err((_, message)) = &result {
        (a.ui)(format!("Ошибка приёма: {message}"));
    }
    result
}
async fn receive_upload(
    a: Arc<App>,
    q: Upload,
    body: Body,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _permit = a.transfers.try_acquire().map_err(|_| {
        (
            StatusCode::CONFLICT,
            "Другая передача ещё выполняется. Повторите отправку.".into(),
        )
    })?;
    if q.sha256.len() != 64 || hex::decode(&q.sha256).is_err() || q.name.len() > 1024 {
        return Err(err("Некорректные сведения о файле"));
    }
    let (part, f) = storage::create_partial(&a.stage).map_err(err)?;
    let mut f = tokio::fs::File::from_std(f);
    let mut stream = body.into_data_stream();
    let mut hash = Sha256::new();
    let mut done = 0u64;
    let mut last = Instant::now();
    let cancel = a.cancel.lock().unwrap().clone();
    loop {
        let chunk = tokio::select! { _ = cancel.cancelled() => return Err(err("Передача отменена")), c = tokio::time::timeout(Duration::from_secs(60), stream.next()) => c.map_err(|_| err("Нет данных 60 секунд. Соединение прервано."))? };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.map_err(err)?;
        done = done
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| err("Size overflow"))?;
        if done > q.size {
            return Err(err("Получено больше данных, чем заявлено"));
        }
        f.write_all(&chunk).await.map_err(err)?;
        hash.update(&chunk);
        if last.elapsed() > Duration::from_millis(250) {
            (a.ui)(format!("Приём: {done} / {} байт", q.size));
            last = Instant::now();
        }
    }
    if done != q.size || hex::encode(hash.finalize()) != q.sha256.to_lowercase() {
        return Err(err(
            "Размер или SHA-256 не совпал. Незавершённый файл удалён.",
        ));
    }
    if cancel.is_cancelled() {
        return Err(err("Передача отменена"));
    }
    f.sync_all().await.map_err(err)?;
    drop(f);
    let _epoch = a.cancel.lock().unwrap();
    if cancel.is_cancelled() {
        return Err(err("Передача отменена"));
    }
    let dest = storage::publish(&part.0, &a.received, &q.name).map_err(err)?;
    (a.ui)(format!(
        "Получено и проверено: {} ({done} байт)",
        dest.display()
    ));
    Ok(Json(
        serde_json::json!({"saved": dest.file_name().unwrap().to_string_lossy()}),
    ))
}
#[derive(Deserialize)]
struct Text {
    text: String,
}
async fn text(
    State(a): State<Arc<App>>,
    h: HeaderMap,
    Json(t): Json<Text>,
) -> Result<StatusCode, ApiError> {
    auth(&a, &h)?;
    if t.text.len() > 1024 * 1024 {
        return Err(err("Text exceeds 1 MiB"));
    }
    (a.ui)(format!("TEXT:{}", t.text));
    Ok(StatusCode::NO_CONTENT)
}
fn router(a: Arc<App>) -> Router {
    Router::new()
        .route("/v1/pair", post(pair))
        .route("/v1/next", get(next))
        .route("/v1/ack/:id", post(ack))
        .route("/v1/file/:id", get(download))
        .route("/v1/upload", post(upload))
        .route("/v1/text", post(text))
        .layer(DefaultBodyLimit::max(1024 * 1024 + 4096))
        .with_state(a)
}
pub async fn serve(a: Arc<App>) -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls =
        axum_server::tls_rustls::RustlsConfig::from_pem(a.cert.clone(), a.key.clone()).await?;
    let router = router(a);
    axum_server::bind_rustls(([0, 0, 0, 0], 45831).into(), tls)
        .serve(router.into_make_service())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;
    struct Fixture {
        app: Arc<App>,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
            let app = App::new(root.clone(), Arc::new(|_| {})).unwrap();
            *app.session.lock().unwrap() = Some("test-session".into());
            Self { app, root }
        }
        async fn upload(&self, bytes: Vec<u8>, size: usize, hash: String) -> Response {
            let request = Request::builder()
                .method("POST")
                .uri(format!("/v1/upload?name=CON.txt&size={size}&sha256={hash}"))
                .header("authorization", "Bearer test-session")
                .body(Body::from(bytes))
                .unwrap();
            router(self.app.clone()).oneshot(request).await.unwrap()
        }
        fn count(&self, dir: &str) -> usize {
            std::fs::read_dir(self.root.join(dir)).unwrap().count()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[tokio::test]
    async fn unauthenticated_cannot_read_or_write() {
        let f = Fixture::new();
        for (method, path) in [
            ("GET", "/v1/next"),
            ("POST", "/v1/upload?name=x&size=0&sha256=00"),
        ] {
            let r = router(f.app.clone())
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
        assert_eq!(f.count("Received"), 0);
        assert_eq!(f.count("Staging"), 0);
    }
    #[tokio::test]
    async fn large_upload_is_verified_and_conflicts_are_preserved() {
        let f = Fixture::new();
        let bytes = vec![0xAB; 3 * 1024 * 1024];
        let hash = hex::encode(Sha256::digest(&bytes));
        for _ in 0..2 {
            assert_eq!(
                f.upload(bytes.clone(), bytes.len(), hash.clone())
                    .await
                    .status(),
                StatusCode::OK
            );
        }
        assert_eq!(f.count("Received"), 2);
        assert_eq!(f.count("Staging"), 0);
        for entry in std::fs::read_dir(&f.app.received).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }
    #[tokio::test]
    async fn corrupt_truncated_and_excess_uploads_never_publish() {
        let f = Fixture::new();
        let hash = hex::encode(Sha256::digest(b"abc"));
        for (bytes, size) in [
            (b"bad".to_vec(), 3),
            (b"ab".to_vec(), 3),
            (b"abcd".to_vec(), 3),
        ] {
            assert_eq!(
                f.upload(bytes, size, hash.clone()).await.status(),
                StatusCode::BAD_REQUEST
            );
            assert_eq!(f.count("Received"), 0);
            assert_eq!(f.count("Staging"), 0);
        }
    }
    #[tokio::test]
    async fn empty_file_is_valid() {
        let f = Fixture::new();
        assert_eq!(
            f.upload(vec![], 0, hex::encode(Sha256::digest([])))
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(f.count("Received"), 1);
    }
    #[tokio::test]
    async fn cancelled_upload_cleans_partial_file() {
        let f = Fixture::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let first = futures_util::stream::once(async move {
            let _ = tx.send(());
            Ok::<_, std::io::Error>(vec![7u8; 65536])
        });
        let body = Body::from_stream(first.chain(futures_util::stream::pending()));
        let a = f.app.clone();
        let task = tokio::spawn(async move {
            receive_upload(
                a,
                Upload {
                    name: "cancelled.bin".into(),
                    size: 131072,
                    sha256: "0".repeat(64),
                },
                body,
            )
            .await
        });
        rx.await.unwrap();
        f.app.cancel_all();
        assert!(task.await.unwrap().is_err());
        assert_eq!(f.count("Received"), 0);
        assert_eq!(f.count("Staging"), 0);
    }
    #[tokio::test]
    async fn revocation_prevents_new_requests() {
        let f = Fixture::new();
        f.app.disconnect();
        let r = f.upload(vec![], 0, hex::encode(Sha256::digest([]))).await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(f.count("Staging"), 0);
    }
    #[tokio::test]
    async fn snapshot_survives_source_change_and_cancel_cleans_it() {
        let f = Fixture::new();
        let source = f.root.join("source.bin");
        std::fs::write(&source, b"original").unwrap();
        f.app.clone().queue_file(source.clone()).await.unwrap();
        std::fs::write(source, b"changed").unwrap();
        let item = f.app.queue.lock().unwrap().front().cloned().unwrap();
        assert_eq!(std::fs::read(item.path.unwrap()).unwrap(), b"original");
        f.app.cancel_all();
        assert!(f.app.queue.lock().unwrap().is_empty());
        assert_eq!(f.count("Staging"), 0);
    }
    #[tokio::test]
    async fn pairing_requires_approval_and_rejects_wrong_secret() {
        let f = Fixture::new();
        *f.app.session.lock().unwrap() = None;
        let denied = pair(
            State(f.app.clone()),
            Json(Pair {
                token: "wrong".into(),
                name: "phone".into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(denied.0, StatusCode::FORBIDDEN);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let root = f.root.join("pairing");
        let app = App::new(
            root,
            Arc::new(move |m| {
                let _ = tx.send(m);
            }),
        )
        .unwrap();
        let task = tokio::spawn({
            let a = app.clone();
            async move {
                pair(
                    State(a.clone()),
                    Json(Pair {
                        token: a.invitation.clone(),
                        name: "phone".into(),
                    }),
                )
                .await
            }
        });
        rx.recv().await.unwrap();
        assert!(app.session.lock().unwrap().is_none());
        app.answer(true);
        assert!(task.await.unwrap().is_ok());
        assert!(app.session.lock().unwrap().is_some());
    }
}
