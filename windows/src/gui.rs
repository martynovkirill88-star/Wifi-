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
    tabs: nwg::TabsContainer,
    transfer: nwg::Tab,
    connection: nwg::Tab,
    journal: nwg::Tab,
    labels: Vec<nwg::Label>,
    heading_font: nwg::Font,
    last_text: RefCell<String>,
    island: super::island::Island,
    autostart: nwg::CheckBox,
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
        let host = host.split(" — ").next().unwrap_or("127.0.0.1");
        let json = serde_json::json!({"v":1,"host":host,"port":45831,"fingerprint":app.fingerprint,"token":app.invitation}).to_string();
        self.code.set_text(&json);
        if let Ok(q) = qrcode::QrCode::new(json.as_bytes()) {
            let modules = q.width() + 8;
            let scale = 260 / modules;
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
        .size((800, 700))
        .position((160, 65))
        .title("LocalDrop")
        .flags(
            nwg::WindowFlags::WINDOW
                | nwg::WindowFlags::MINIMIZE_BOX
                | if std::env::args().any(|a| a == "--tray") {
                    nwg::WindowFlags::empty()
                } else {
                    nwg::WindowFlags::VISIBLE
                },
        )
        .build(&mut w.window)?;
    nwg::Notice::builder()
        .parent(&w.window)
        .build(&mut w.notice)?;
    nwg::Font::builder()
        .family("Segoe UI")
        .size(26)
        .weight(650)
        .build(&mut w.heading_font)?;
    macro_rules! label {
        ($parent:expr,$text:expr,$x:expr,$y:expr,$width:expr,$height:expr) => {{
            let mut label = nwg::Label::default();
            nwg::Label::builder()
                .parent($parent)
                .text($text)
                .position(($x, $y))
                .size(($width, $height))
                .build(&mut label)?;
            w.labels.push(label);
        }};
    }
    let mut title = nwg::Label::default();
    nwg::Label::builder()
        .parent(&w.window)
        .text("LocalDrop")
        .font(Some(&w.heading_font))
        .position((24, 15))
        .size((280, 42))
        .build(&mut title)?;
    w.labels.push(title);
    nwg::Label::builder()
        .parent(&w.window)
        .position((24, 65))
        .size((750, 42))
        .text(
            "Подключите iPhone на вкладке «Подключение». Файлы передаются напрямую по вашей сети.",
        )
        .build(&mut w.status)?;
    nwg::TabsContainer::builder()
        .parent(&w.window)
        .position((20, 115))
        .size((760, 555))
        .build(&mut w.tabs)?;
    nwg::Tab::builder()
        .parent(&w.tabs)
        .text("Передача")
        .build(&mut w.transfer)?;
    nwg::Tab::builder()
        .parent(&w.tabs)
        .text("Подключение")
        .build(&mut w.connection)?;
    nwg::Tab::builder()
        .parent(&w.tabs)
        .text("Журнал")
        .build(&mut w.journal)?;
    macro_rules! button {
        ($field:ident,$parent:expr,$text:expr,$x:expr,$y:expr,$width:expr) => {
            nwg::Button::builder()
                .parent($parent)
                .text($text)
                .position(($x, $y))
                .size(($width, 34))
                .build(&mut w.$field)?;
        };
    }
    button!(folder, &w.window, "Открыть полученные", 450, 20, 200);
    button!(quit, &w.window, "Выход", 665, 20, 110);
    label!(&w.transfer, "ФАЙЛЫ", 20, 20, 300, 22);
    button!(files, &w.transfer, "Отправить файлы…", 20, 50, 230);
    button!(cancel, &w.transfer, "Отменить передачи", 265, 50, 225);
    label!(&w.transfer, "ТЕКСТ НА IPHONE", 20, 110, 500, 22);
    nwg::TextBox::builder()
        .parent(&w.transfer)
        .position((20, 140))
        .size((710, 105))
        .limit(1024 * 1024)
        .build(&mut w.text)?;
    button!(paste, &w.transfer, "Вставить из буфера", 20, 258, 230);
    button!(send, &w.transfer, "Отправить текст", 265, 258, 225);
    label!(
        &w.transfer,
        "ПОЛУЧЕННЫЙ ТЕКСТ · автоматически копируется в буфер",
        20,
        315,
        710,
        24
    );
    nwg::TextBox::builder()
        .parent(&w.transfer)
        .position((20, 345))
        .size((710, 90))
        .readonly(true)
        .limit(1024 * 1024)
        .build(&mut w.received)?;
    button!(copy, &w.transfer, "Копировать ещё раз", 20, 448, 230);
    label!(&w.connection, "СЕТЕВОЙ АДРЕС КОМПЬЮТЕРА", 20, 20, 710, 22);
    let mut addresses = localdrop::network::addresses()?;
    if addresses.is_empty() {
        addresses.push("127.0.0.1 — сеть не найдена".into());
    }
    nwg::ComboBox::builder()
        .parent(&w.connection)
        .position((20, 48))
        .size((710, 28))
        .collection(addresses)
        .selected_index(Some(0))
        .build(&mut w.address)?;
    nwg::ImageFrame::builder()
        .parent(&w.connection)
        .position((20, 95))
        .size((260, 260))
        .build(&mut w.qr)?;
    label!(&w.connection,"1. Откройте LocalDrop на iPhone.\r\n2. Сканируйте этот QR-код.\r\n3. При появлении запроса нажмите «Разрешить».\r\n\r\nСопряжение запоминается. Не передавайте код посторонним.",305,95,425,115);
    label!(
        &w.connection,
        "КОД ДЛЯ РУЧНОГО ПОДКЛЮЧЕНИЯ",
        305,
        220,
        425,
        22
    );
    nwg::TextBox::builder()
        .parent(&w.connection)
        .position((305, 248))
        .size((425, 107))
        .readonly(true)
        .build(&mut w.code)?;
    button!(allow, &w.connection, "Разрешить", 20, 375, 125);
    button!(deny, &w.connection, "Отклонить", 155, 375, 125);
    button!(
        disconnect,
        &w.connection,
        "Забыть устройство",
        305,
        375,
        210
    );
    w.allow.set_enabled(false);
    w.deny.set_enabled(false);
    nwg::CheckBox::builder()
        .parent(&w.connection)
        .text("Запускать вместе с Windows (в трей)")
        .position((20, 495))
        .size((710, 24))
        .build(&mut w.autostart)?;
    label!(&w.connection,"Выбирайте Ethernet или Wi-Fi домашней сети. VPN и виртуальные адаптеры могут быть недоступны с телефона.",20,430,710,60);
    label!(&w.journal, "ПОЛУЧЕННЫЕ ФАЙЛЫ", 20, 20, 710, 22);
    let received_path = localdrop::storage::documents_received()?;
    label!(
        &w.journal,
        &received_path.display().to_string(),
        20,
        50,
        710,
        45
    );
    label!(&w.journal, "ИСТОРИЯ ПЕРЕДАЧ", 20, 110, 710, 22);
    nwg::TextBox::builder()
        .parent(&w.journal)
        .position((20, 140))
        .size((710, 345))
        .readonly(true)
        .build(&mut w.history)?;
    w.tabs.set_selected_tab(1);
    w.island = super::island::Island::build(&w.window)?;
    nwg::Icon::builder()
        .source_bin(Some(include_bytes!("../app.ico")))
        .size(Some((32, 32)))
        .build(&mut w.icon)?;
    w.window.set_icon(Some(&w.icon));
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
    let startup_root = root.clone();
    let start_enabled = super::startup::enabled(&root);
    w.autostart.set_check_state(if start_enabled {
        nwg::CheckBoxState::Checked
    } else {
        nwg::CheckBoxState::Unchecked
    });
    if std::env::var_os("CI").is_none() {
        if let Err(e) = super::startup::set(&root, start_enabled) {
            w.status.set_text(&e.to_string());
        }
    }
    use std::os::windows::fs::OpenOptionsExt;
    let _instance = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(root.join("instance.lock"))?;
    let app = App::persistent(
        root,
        localdrop::storage::documents_received()?,
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
    let discovery = match localdrop::network::advertise(&app.fingerprint) {
        Ok(d) => Some(d),
        Err(e) => {
            (app.ui)(format!(
                "Автообнаружение недоступно: {e}. Подключение по QR/IP доступно."
            ));
            None
        }
    };
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
    if app.session.lock().unwrap().is_some() {
        w.tabs.set_selected_tab(0);
        w.status.set_text("Устройство сохранено. Откройте LocalDrop на iPhone — соединение восстановится автоматически.");
    }
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
                let pending = app.pending.lock().unwrap().is_some();
                w.allow.set_enabled(pending);
                w.deny.set_enabled(pending);
                if pending {
                    w.window.set_visible(true);
                    w.tabs.set_selected_tab(1);
                }
                for message in messages.lock().unwrap().drain(..) {
                    if let Some(text) = message.strip_prefix("TEXT:") {
                        w.received.set_text(text);
                        *w.last_text.borrow_mut() = text.to_owned();
                        nwg::Clipboard::set_data_text(&w.window, text);
                        w.status.set_text(
                            if nwg::Clipboard::data_text(&w.window).as_deref() == Some(text) {
                                "Текст получен и скопирован в буфер обмена"
                            } else {
                                "Текст получен. Буфер занят — нажмите «Копировать»."
                            },
                        );
                    } else if let Some(progress) = message
                        .strip_prefix("TRANSFER:")
                        .and_then(|s| serde_json::from_str::<localdrop::server::Progress>(s).ok())
                    {
                        w.island.update(&progress);
                        w.status.set_text(&progress.description());
                    } else {
                        w.status.set_text(&message);
                        if message.starts_with("Сопряжено:") {
                            w.tabs.set_selected_tab(0);
                        }
                        if message.contains("отменен")
                            || message.contains("отменён")
                            || message.starts_with("Ошибка")
                        {
                            w.island.finish(&message);
                        }
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
            if event == E::OnTimerTick && handle == w.island.timer {
                w.island.tick();
            }
            if event == E::OnComboxBoxSelection && handle == w.address.handle {
                w.show_qr(&app);
                w.status.set_text("Адрес изменён. Сканируйте обновлённый QR на iPhone. Выбирайте Ethernet или Wi-Fi своей сети, а не VPN.");
            }
            if event != E::OnButtonClick {
                return;
            }
            if handle == w.autostart.handle {
                if let Err(e) = super::startup::set(
                    &startup_root,
                    w.autostart.check_state() == nwg::CheckBoxState::Checked,
                ) {
                    w.status.set_text(&e.to_string());
                }
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
                nwg::Clipboard::set_data_text(&w.window, &w.last_text.borrow());
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
                    index: 1,
                    count: 1,
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
                            let count = paths.len() as u32;
                            for (index, path) in paths.into_iter().enumerate() {
                                if batch_cancel.is_cancelled() {
                                    break;
                                }
                                if let Err(e) = a
                                    .clone()
                                    .queue_file_batch(path.into(), index as u32 + 1, count)
                                    .await
                                {
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
    app.cancel_all();
    if let Some(d) = discovery {
        let _ = d.shutdown();
    }
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    Ok(())
}
