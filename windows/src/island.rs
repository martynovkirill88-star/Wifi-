use localdrop::server::Progress;
use native_windows_gui as nwg;
use std::{cell::RefCell, time::Instant};
use windows_sys::Win32::{
    Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn},
    UI::WindowsAndMessaging::{KillTimer, SetLayeredWindowAttributes, SetTimer, LWA_ALPHA},
};

#[derive(Default)]
pub struct Island {
    window: nwg::Window,
    title: nwg::Label,
    detail: nwg::Label,
    bar: nwg::ProgressBar,
    pub timer: nwg::ControlHandle,
    hide_at: RefCell<Option<Instant>>,
}
impl Island {
    pub fn build(parent: &nwg::Window) -> Result<Self, nwg::NwgError> {
        let mut s = Self::default();
        nwg::Window::builder()
            .title("LocalDrop Transfer")
            .size((440, 104))
            .position(((nwg::Monitor::width() - 440) / 2, 14))
            .flags(nwg::WindowFlags::POPUP)
            .ex_flags(0x08080088)
            .build(&mut s.window)?;
        nwg::Label::builder()
            .parent(&s.window)
            .text("LocalDrop")
            .position((22, 14))
            .size((396, 24))
            .build(&mut s.title)?;
        nwg::Label::builder()
            .parent(&s.window)
            .position((22, 40))
            .size((396, 23))
            .build(&mut s.detail)?;
        nwg::ProgressBar::builder()
            .parent(&s.window)
            .position((22, 72))
            .size((396, 8))
            .range(0..1000)
            .build(&mut s.bar)?;
        s.timer = nwg::ControlHandle::Timer(parent.handle.hwnd().unwrap(), 0x4c44);
        unsafe {
            let hwnd = s.window.handle.hwnd().unwrap() as _;
            SetWindowRgn(hwnd, CreateRoundRectRgn(0, 0, 441, 105, 28, 28), 1);
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        }
        Ok(s)
    }
    pub fn update(&self, p: &Progress) {
        self.stop_timer();
        *self.hide_at.borrow_mut() = None;
        unsafe {
            SetLayeredWindowAttributes(self.window.handle.hwnd().unwrap() as _, 0, 255, LWA_ALPHA);
        }
        self.title.set_text(&format!(
            "{} · файл {} из {}",
            p.direction, p.index, p.count
        ));
        self.detail.set_text(&format!(
            "{:.1} / {:.1} МБ · {}",
            p.done as f64 / 1048576.0,
            p.total as f64 / 1048576.0,
            p.name
        ));
        self.bar.set_pos(if p.total == 0 {
            if p.complete {
                1000
            } else {
                0
            }
        } else {
            ((p.done as f64 / p.total as f64) * 1000.0).min(1000.0) as u32
        });
        self.window.set_visible(true);
        if p.complete && p.index == p.count {
            self.finish("Передача завершена");
        }
    }
    pub fn finish(&self, message: &str) {
        self.title.set_text(message);
        *self.hide_at.borrow_mut() = Some(Instant::now());
        if let nwg::ControlHandle::Timer(hwnd, id) = self.timer {
            unsafe {
                SetTimer(hwnd as _, id as usize, 30, None);
            }
        }
    }
    pub fn tick(&self) {
        let Some(start) = *self.hide_at.borrow() else {
            self.stop_timer();
            return;
        };
        let fade = (start.elapsed().as_secs_f64() - 1.8) / 0.35;
        if fade >= 1.0 {
            self.window.set_visible(false);
            self.stop_timer();
            *self.hide_at.borrow_mut() = None;
        } else if fade > 0.0 {
            unsafe {
                SetLayeredWindowAttributes(
                    self.window.handle.hwnd().unwrap() as _,
                    0,
                    ((1.0 - fade) * 255.0) as u8,
                    LWA_ALPHA,
                );
            }
        }
    }
    fn stop_timer(&self) {
        if let nwg::ControlHandle::Timer(hwnd, id) = self.timer {
            unsafe {
                KillTimer(hwnd as _, id as usize);
            }
        }
    }
}
impl Drop for Island {
    fn drop(&mut self) {
        self.stop_timer();
    }
}
