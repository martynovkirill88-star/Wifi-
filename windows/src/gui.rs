use localdrop::server::{App, Item};
use native_windows_gui as nwg;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Window {
    window: nwg::Window,
    notice: nwg::Notice,
    address: nwg::ComboBox<String>,
    qr: nwg::ImageFrame,
    code: nwg::TextBox,
    status: nwg::Label,
    allow: nwg::Button,
    deny: nwg::Button,
    files: nwg::Button,
    send: nwg::Button,
    cancel: nwg::Button,
    folder: nwg::Button,
    disconnect: nwg::Button,
    quit: nwg::Button,
    text: nwg::TextBox,
    received: nwg::TextBox,
    history: nwg::TextBox,
    history_lines: RefCell<std::collections::VecDeque<String>>,
    paste: nwg::Button,
    copy: nwg::Button,
    tray: nwg::TrayNotification,
    icon: nwg::Icon,
    bitmap: RefCell<Option<nwg::Bitmap>>,
}
impl Window {
    fn show_qr(&self, app: &App) {
        let host = self
            .address
            .selection_string()
            .unwrap_or_else(|| "127.0.0.1".into());
        let json = serde_json::json!({"v":1,"host":host,"port":45831,"fingerprint":app.fingerprint,"token":app.invitation}).to_string();
        self.code.set_text(&json);
        if let Ok(q) = qrcode::QrCode::new(json.as_bytes()) {
            let modules = q.width() + 8;
            let scale = 300 / modules;
            let width = modules * scale;
            let mut pixels = vec![255u8; width * width];
            for y in 0..q.width() {
                for x in 0..q.width() {
                    if q[(x, y)] == qrcode::Color::Dark {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                pixels[((y + 4) * scale + dy) * width + (x + 4) * scale + dx] = 0;
                            }
                        }
                    }
                }
            }
            let mut png = Vec::new();
            let encoded = (|| -> Result<(), png::EncodingError> {
                let mut encoder = png::Encoder::new(&mut png, width as u32, width as u32);
                encoder.set_color(png::ColorType::Grayscale);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header()?.write_image_data(&pixels)
            })();
            if encoded.is_ok() {
                if let Ok(b) = nwg::Bitmap::from_bin(&png) {
                    self.qr.set_bitmap(Some(&b));
                    *self.bitmap.borrow_mut() = Some(b);
                }
            }
        }
    }
}
pub fn run() -> anyhow::Result<()> {
    nwg::init()?;
    nwg::Font::set_global_family("Segoe UI")?;
    let mut w = Window::default();
    nwg::Window::builder()
        .size((800, 890))
        .position((200, 80))
        .title("LocalDrop — передача по локальной сети")
        .build(&mut w.window)?;
    nwg::Notice::builder()
        .parent(&w.window)
        .build(&mut w.notice)?;
    let mut addresses: Vec<String> = if_addrs::get_if_addrs()?
        .into_iter()
        .filter_map(|a| match a.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
            _ => None,
        })
        .collect();
    addresses.sort();
    addresses.dedup();
    if addresses.is_empty() {
        addresses.push("127.0.0.1".into());
    }
    nwg::ComboBox::builder()
        .parent(&w.window)
        .position((20, 15))
        .size((310, 28))
        .collection(addresses)
        .selected_index(Some(0))
        .build(&mut w.address)?;
    nwg::ImageFrame::builder()
        .parent(&w.window)
        .position((20, 55))
        .size((300, 300))
        .build(&mut w.qr)?;
    nwg::TextBox::builder()
        .parent(&w.window)
        .position((340, 55))
        .size((435, 190))
        .readonly(true)
        .build(&mut w.code)?;
    nwg::Label::builder().parent(&w.window).position((340,250)).size((435,80)).text("Выберите IP своей Wi-Fi сети. Сканируйте QR или вставьте весь код вручную на iPhone. Не передавайте код посторонним.").build(&mut w.status)?;
    macro_rules! button {
        ($field:ident,$text:expr,$x:expr,$y:expr,$width:expr) => {
            nwg::Button::builder()
                .parent(&w.window)
                .text($text)
                .position(($x, $y))
                .size(($width, 32))
                .build(&mut w.$field)?;
        };
    }
    button!(allow, "Разрешить", 20, 365, 145);
    button!(deny, "Отклонить", 175, 365, 145);
    button!(disconnect, "Отключить устройство", 340, 365, 210);
    button!(quit, "Выход", 565, 365, 210);
    button!(files, "Отправить файлы…", 20, 410, 230);
    button!(cancel, "Отменить передачи", 265, 410, 245);
    button!(folder, "Открыть полученные", 525, 410, 250);
    nwg::TextBox::builder()
        .parent(&w.window)
        .position((20, 455))
        .size((755, 110))
        .build(&mut w.text)?;
    button!(paste, "Вставить текст", 20, 575, 230);
    button!(send, "Отправить текст", 265, 575, 245);
    nwg::TextBox::builder()
        .parent(&w.window)
        .position((20, 617))
        .size((755, 90))
        .readonly(true)
        .build(&mut w.received)?;
    button!(copy, "Копировать полученный текст", 20, 718, 320);
    nwg::TextBox::builder()
        .parent(&w.window)
        .position((20, 760))
        .size((755, 110))
        .readonly(true)
        .build(&mut w.history)?;
    w.icon = nwg::Icon::from_system(nwg::OemIcon::Information);
    nwg::TrayNotification::builder()
        .parent(&w.window)
        .icon(Some(&w.icon))
        .tip(Some("LocalDrop — нажмите, чтобы открыть"))
        .build(&mut w.tray)?;
    let notice = w.notice.sender();
    let messages = Arc::new(Mutex::new(std::collections::VecDeque::<String>::new()));
    let events = messages.clone();
    let root = PathBuf::from(
        std::env::var_os("LOCALAPPDATA").ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA missing"))?,
    )
    .join("LocalDrop");
    // Single instance: holding a non-shared Windows file handle also guards staging cleanup.
    std::fs::create_dir_all(&root)?;
    use std::os::windows::fs::OpenOptionsExt;
    let _instance = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(root.join("instance.lock"))?;
    let app = App::new(
        root,
        Arc::new(move |m| {
            let mut q = events.lock().unwrap();
            if q.len() >= 64 {
                q.pop_front();
            }
            q.push_back(m);
            drop(q);
            notice.notice();
        }),
    )?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    runtime.spawn({
        let a = app.clone();
        async move {
            if let Err(e) = localdrop::server::serve(a.clone()).await {
                (a.ui)(format!(
                    "Не удалось запустить сервер: {e}. Возможно, порт 45831 занят."
                ));
            }
        }
    });
    let w = Rc::new(w);
    w.show_qr(&app);
    let handler = nwg::full_bind_event_handler(&w.window.handle, {
        let w = w.clone();
        let app = app.clone();
        let rt = runtime.handle().clone();
        move |event, _, handle| {
            use nwg::Event as E;
            if event == E::OnWindowClose {
                w.window.set_visible(false);
            }
            if event == E::OnMousePress(nwg::MousePressEvent::MousePressLeftUp)
                && handle == w.tray.handle
            {
                w.window.set_visible(true);
                w.window.set_focus();
            }
            if event == E::OnNotice && handle == w.notice.handle {
                for message in messages.lock().unwrap().drain(..) {
                    if let Some(text) = message.strip_prefix("TEXT:") {
                        w.received.set_text(text);
                        w.status.set_text("Получен текст");
                    } else {
                        w.status.set_text(&message);
                        if !message.starts_with("Отправка:")
                            && !message.starts_with("Приём:")
                            && !message.starts_with("Подготовка:")
                        {
                            let mut lines = w.history_lines.borrow_mut();
                            if lines.len() >= 100 {
                                lines.pop_front();
                            }
                            lines.push_back(message);
                            w.history
                                .set_text(&lines.iter().cloned().collect::<Vec<_>>().join("\r\n"));
                        }
                    }
                }
            }
            if event == E::OnComboxBoxSelection && handle == w.address.handle {
                w.show_qr(&app);
            }
            if event != E::OnButtonClick {
                return;
            }
            if handle == w.allow.handle {
                app.answer(true);
            }
            if handle == w.deny.handle {
                app.answer(false);
            }
            if handle == w.quit.handle {
                app.cancel_all();
                nwg::stop_thread_dispatch();
            }
            if handle == w.disconnect.handle {
                app.disconnect();
                w.status
                    .set_text("Отключено. Для подключения сканируйте QR снова.");
            }
            if handle == w.cancel.handle {
                app.cancel_all();
            }
            if handle == w.folder.handle {
                let _ = std::process::Command::new("explorer.exe")
                    .arg(&app.received)
                    .spawn();
            }
            if handle == w.paste.handle {
                if let Some(t) = nwg::Clipboard::data_text(&w.window) {
                    w.text.set_text(&t);
                }
            }
            if handle == w.copy.handle {
                nwg::Clipboard::set_data_text(&w.window, &w.received.text());
            }
            if handle == w.send.handle {
                let text = w.text.text();
                if text.len() > 1024 * 1024 {
                    w.status.set_text("Текст превышает 1 МиБ");
                    return;
                }
                let item = Item {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: "Текст".into(),
                    size: text.len() as u64,
                    sha256: String::new(),
                    text: Some(text),
                    path: None,
                };
                match app.enqueue(item) {
                    Ok(()) => w.status.set_text("Текст в очереди"),
                    Err(e) => w.status.set_text(&e.to_string()),
                }
            }
            if handle == w.files.handle {
                let mut dialog = nwg::FileDialog::default();
                if nwg::FileDialog::builder()
                    .action(nwg::FileDialogAction::Open)
                    .multiselect(true)
                    .build(&mut dialog)
                    .is_ok()
                    && dialog.run(Some(&w.window))
                {
                    if let Ok(paths) = dialog.get_selected_items() {
                        let a = app.clone();
                        let batch_cancel = a.cancel.lock().unwrap().clone();
                        rt.spawn(async move {
                            for path in paths {
                                if batch_cancel.is_cancelled() {
                                    break;
                                }
                                if let Err(e) = a.clone().queue_file(path.into()).await {
                                    (a.ui)(format!("Ошибка подготовки файла: {e}"));
                                    break;
                                }
                            }
                        });
                    }
                }
            }
        }
    });
    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
    app.disconnect();
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    Ok(())
}
