use localdrop::server::Progress;
use native_windows_gui as nwg;
use std::{cell::{Cell, RefCell}, rc::Rc, time::Instant};
use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn, CreateSolidBrush, DeleteObject, FillRect, BeginPaint, EndPaint, PAINTSTRUCT, SetTextColor, SetBkMode, TRANSPARENT, InvalidateRect},
    UI::WindowsAndMessaging::{KillTimer, SetLayeredWindowAttributes, SetTimer, LWA_ALPHA, WM_PAINT, WM_ERASEBKGND, WM_CTLCOLORSTATIC, GetClientRect},
};

#[derive(Default)]
pub struct Island {
    window: nwg::Window,
    title: nwg::Label,
    detail: nwg::Label,
    logo: nwg::Label,
    logo_font: nwg::Font,
    percent: Rc<Cell<u32>>,
    paint_handler: Option<nwg::RawEventHandler>,
    pub timer: nwg::ControlHandle,
    hide_at: RefCell<Option<Instant>>,
}
impl Island {
    pub fn build(parent: &nwg::Window) -> Result<Self, nwg::NwgError> {
        let mut s = Self::default();
        nwg::Window::builder()
            .title("LocalDrop Transfer")
            .size((480, 82))
            .position(((nwg::Monitor::width() - 480) / 2, 14))
            .flags(nwg::WindowFlags::POPUP)
            .ex_flags(0x08080088)
            .build(&mut s.window)?;
        nwg::Label::builder()
            .parent(&s.window)
            .text("LocalDrop")
            .position((76, 13))
            .size((382, 23))
            .build(&mut s.title)?;
        nwg::Label::builder()
            .parent(&s.window)
            .position((76, 36))
            .size((382, 21))
            .build(&mut s.detail)?;
        nwg::Font::builder().family("Segoe UI Symbol").size(31).build(&mut s.logo_font)?;
        nwg::Label::builder().parent(&s.window).font(Some(&s.logo_font)).text("⇄").position((21,18)).size((44,42)).build(&mut s.logo)?;
        let progress = s.percent.clone();
        let logo_hwnd = s.logo.handle.hwnd().unwrap() as isize;
        s.paint_handler = Some(nwg::bind_raw_event_handler(&s.window.handle,0x10044,move |hwnd,msg,w,l| unsafe {
            if msg == WM_CTLCOLORSTATIC {
                let hdc = w as _;
                SetTextColor(hdc,if l == logo_hwnd { 0x00dbe255 } else { 0x00fafafa });
                SetBkMode(hdc,TRANSPARENT as i32);
                // Stock black brush is owned by the OS.
                return Some(windows_sys::Win32::Graphics::Gdi::GetStockObject(windows_sys::Win32::Graphics::Gdi::BLACK_BRUSH) as isize);
            }
            if msg == WM_PAINT || msg == WM_ERASEBKGND {
                let mut ps:PAINTSTRUCT = std::mem::zeroed();
                let hdc = if msg == WM_PAINT { BeginPaint(hwnd as _,&mut ps) } else { w as _ };
                let mut rect:RECT = std::mem::zeroed(); GetClientRect(hwnd as _,&mut rect);
                let black = CreateSolidBrush(0); FillRect(hdc,&rect,black); DeleteObject(black);
                let track = RECT {left:76,top:64,right:456,bottom:67};
                let grey = CreateSolidBrush(0x00242424); FillRect(hdc,&track,grey); DeleteObject(grey);
                let fill = RECT {right:76+(380*progress.get()/1000) as i32,..track};
                let cyan = CreateSolidBrush(0x00dbe255); FillRect(hdc,&fill,cyan); DeleteObject(cyan);
                if msg == WM_PAINT { EndPaint(hwnd as _,&ps); }
                return Some(if msg == WM_ERASEBKGND {1} else {0});
            }
            None
        })?);
        s.timer = nwg::ControlHandle::Timer(parent.handle.hwnd().unwrap(), 0x4c44);
        unsafe {
            let hwnd = s.window.handle.hwnd().unwrap() as _;
            SetWindowRgn(hwnd, CreateRoundRectRgn(0, 0, 481, 83, 82, 82), 1);
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
        self.percent.set(if p.total == 0 {
            if p.complete {
                1000
            } else {
                0
            }
        } else {
            ((p.done as f64 / p.total as f64) * 1000.0).min(1000.0) as u32
        });
        unsafe { InvalidateRect(self.window.handle.hwnd().unwrap() as _,std::ptr::null(),1); }
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
        if let Some(handler) = self.paint_handler.take() { let _ = nwg::unbind_raw_event_handler(&handler); }
    }
}
