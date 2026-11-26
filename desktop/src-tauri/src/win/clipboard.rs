//! Windows panosu: olay tabanlı dinleme (AddClipboardFormatListener), metin okuma/yazma.
//! Sürekli yoklama yok; boşta CPU ≈ 0.
use std::{
    sync::OnceLock,
    thread,
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{
                AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
            },
            LibraryLoader::GetModuleHandleW,
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
            Ole::CF_UNICODETEXT,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, RegisterClassW,
            WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
        },
    },
    core::{PCWSTR, w},
};

type Callback = Box<dyn Fn() + Send + Sync>;
static ON_CHANGE: OnceLock<Callback> = OnceLock::new();

/// Pano açık tutulan başka bir uygulama yüzünden kısa süreli başarısızlıklara karşı tekrar dener.
fn open() -> bool {
    for _ in 0..20 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Parola yöneticileri gibi uygulamaların "izlemeyin" işaretini taşıyan içerik mi?
fn excluded() -> bool {
    unsafe {
        let f = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        f != 0 && IsClipboardFormatAvailable(f).is_ok()
    }
}

pub fn get_text() -> Option<String> {
    if !open() {
        return None;
    }
    let out = unsafe {
        if excluded() {
            None
        } else {
            match GetClipboardData(CF_UNICODETEXT.0 as u32) {
                Ok(h) if !h.is_invalid() => {
                    let g = HGLOBAL(h.0);
                    let p = GlobalLock(g) as *const u16;
                    if p.is_null() {
                        None
                    } else {
                        let max = GlobalSize(g) / 2;
                        let mut len = 0usize;
                        while len < max && *p.add(len) != 0 {
                            len += 1;
                        }
                        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                        let _ = GlobalUnlock(g);
                        Some(s)
                    }
                }
                _ => None,
            }
        }
    };
    unsafe {
        let _ = CloseClipboard();
    }
    out
}

pub fn set_text(s: &str) -> bool {
    // Windows panosu satır sonu olarak CRLF bekler.
    let normalized = s.replace("\r\n", "\n").replace('\n', "\r\n");
    let wide: Vec<u16> = normalized.encode_utf16().chain(std::iter::once(0)).collect();
    if !open() {
        return false;
    }
    let ok = unsafe {
        (|| -> windows::core::Result<()> {
            EmptyClipboard()?;
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2)?;
            let p = GlobalLock(mem) as *mut u16;
            if p.is_null() {
                let err = windows::core::Error::from_thread();
                let _ = GlobalFree(Some(mem));
                return Err(err);
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
            let _ = GlobalUnlock(mem);
            // Başarılı olursa bellek sahipliği sisteme geçer; başarısızsa biz serbest bırakırız.
            if let Err(e) = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(mem.0))) {
                let _ = GlobalFree(Some(mem));
                return Err(e);
            }
            Ok(())
        })()
        .is_ok()
    };
    unsafe {
        let _ = CloseClipboard();
    }
    ok
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        if let Some(cb) = ON_CHANGE.get() {
            cb();
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Gizli bir "message-only" pencere ile pano değişikliklerini dinler. Bir kez çağrılır.
pub fn spawn_listener(on_change: impl Fn() + Send + Sync + 'static) {
    if ON_CHANGE.set(Box::new(on_change)).is_err() {
        return;
    }
    thread::Builder::new()
        .name("kopru-clipboard".into())
        .spawn(|| unsafe {
            let Ok(hinst) = GetModuleHandleW(None) else { return };
            let class = w!("KopruClipboardListener");
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst.into(), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc);
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(hinst.into()),
                None,
            ) else {
                log::warn!("pano penceresi oluşturulamadı");
                return;
            };
            if AddClipboardFormatListener(hwnd).is_err() {
                log::warn!("pano dinleyicisi eklenemedi");
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
        })
        .expect("pano iş parçacığı");
}
