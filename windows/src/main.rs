#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod gui;
#[cfg(windows)]
mod island;
fn main() {
    #[cfg(windows)]
    if let Err(e) = gui::run() {
        native_windows_gui::simple_message("LocalDrop", &format!("Ошибка запуска: {e:#}"));
    }
    #[cfg(not(windows))]
    eprintln!("Windows GUI requires Windows");
}
