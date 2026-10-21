//! Windows notification-area icon and close-to-tray behaviour.
//!
//! Implemented with `Shell_NotifyIconW` on a dedicated message-only window thread so
//! the egui/eframe event loop stays untouched. Menu clicks are forwarded to the app
//! through a channel; the app then drives the viewport (show/focus/close) with eframe
//! viewport commands.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;

use crossbeam_channel::{Receiver, Sender};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, ShowWindow, TrackPopupMenu, TranslateMessage, MSG, SW_SHOW,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_APP, WM_CLOSE, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
    WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{IDI_APPLICATION, MF_STRING};

/// Events delivered to the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Show,
    Quit,
}

static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);
static EVENTS: OnceLock<Sender<TrayEvent>> = OnceLock::new();
static TRAY_WINDOW: AtomicIsize = AtomicIsize::new(0);
const TRAY_ICON_ID: u32 = 1;
const WM_TRAY_CALLBACK: u32 = WM_APP + 1;

/// Create the tray icon. Returns the event receiver on success.
///
/// `main_window` is the raw HWND of the egui window (used to restore focus).
pub fn init(main_window: isize) -> Option<Receiver<TrayEvent>> {
    MAIN_WINDOW.store(main_window, Ordering::SeqCst);
    let (tx, rx) = crossbeam_channel::unbounded();
    let _ = EVENTS.set(tx);

    match std::thread::Builder::new()
        .name("mhub-tray".into())
        .spawn(|| {
            if run_message_loop().is_none() {
                log::warn!("tray message loop failed to start");
            }
        })
    {
        Ok(_) => Some(rx),
        Err(err) => {
            log::warn!("cannot start tray thread: {err}");
            None
        }
    }
}

fn run_message_loop() -> Option<()> {
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let class_name = w!("MHubNativeTrayWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        let atom = RegisterClassW(&wc);
        if atom == 0 {
            log::warn!(
                "RegisterClassW failed: {}",
                windows::core::Error::from_thread()
            );
            return None;
        }

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("M HUB Native Tray"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()?;
        log::info!("tray window created: {hwnd:?}");
        TRAY_WINDOW.store(hwnd.0 as isize, Ordering::SeqCst);

        let icon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY_CALLBACK,
            hIcon: icon,
            ..Default::default()
        };
        let tip: Vec<u16> = "M HUB Native"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        for (slot, value) in data.szTip.iter_mut().zip(tip.iter()) {
            *slot = *value;
        }
        let added = Shell_NotifyIconW(NIM_ADD, &data).as_bool();
        log::info!("Shell_NotifyIcon(NIM_ADD) ok={added}");

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).into() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
    Some(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_TRAY_CALLBACK {
        let event = (lparam.0 & 0xFFFF) as u32;
        match event {
            WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                focus_main_window();
                send(TrayEvent::Show);
            }
            WM_RBUTTONUP => show_menu(hwnd),
            _ => {}
        }
        return LRESULT(0);
    }
    if message == WM_CLOSE {
        PostQuitMessage(0);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn send(event: TrayEvent) {
    if let Some(tx) = EVENTS.get() {
        let _ = tx.send(event);
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let _ = AppendMenuW(menu, MF_STRING, 1, w!("Open M HUB"));
    let _ = AppendMenuW(menu, MF_STRING, 2, w!("Exit"));
    let mut point = POINT::default();
    let _ = GetCursorPos(&mut point);
    let _ = SetForegroundWindow(hwnd);
    let command = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN,
        point.x,
        point.y,
        None,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
    match command.0 {
        1 => {
            focus_main_window();
            send(TrayEvent::Show);
        }
        2 => {
            send(TrayEvent::Quit);
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(hwnd),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
        }
        _ => {}
    }
}

/// Bring the main window back to the foreground (called on the UI thread).
pub fn focus_main_window() {
    let hwnd = MAIN_WINDOW.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let hwnd = HWND(hwnd as *mut std::ffi::c_void);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}
