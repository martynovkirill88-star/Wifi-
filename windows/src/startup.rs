use anyhow::{ensure, Result};
use std::path::Path;
use windows_sys::Win32::System::Registry::*;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn enabled(root: &Path) -> bool {
    std::fs::read(root.join("autostart.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<bool>(&b).ok())
        .unwrap_or(true)
}
pub fn set(root: &Path, enabled: bool) -> Result<()> {
    let mut key = std::ptr::null_mut();
    let path = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let name = wide("LocalDrop");
    unsafe {
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        ensure!(
            status == 0,
            "Не удалось открыть автозапуск Windows: {status}"
        );
        let result = if enabled {
            let command = wide(&format!(
                "\"{}\" --tray",
                std::env::current_exe()?.display()
            ));
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, name.as_ptr())
        };
        RegCloseKey(key);
        ensure!(
            result == 0 || (!enabled && result == 2),
            "Не удалось изменить автозапуск: {result}"
        );
    }
    std::fs::write(root.join("autostart.json"), serde_json::to_vec(&enabled)?)?;
    Ok(())
}
