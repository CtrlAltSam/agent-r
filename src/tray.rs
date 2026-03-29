use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallWindowProcW, CreateIconIndirect, CreatePopupMenu, DestroyIcon,
    DestroyMenu, GetCursorPos, ICONINFO, IDI_APPLICATION, LoadIconW, MF_STRING, PostMessageW,
    SetForegroundWindow, SetWindowLongPtrW, TrackPopupMenu, GWLP_WNDPROC, TPM_RIGHTBUTTON,
    WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_RBUTTONUP,
};

const WM_TRAYICON: u32 = WM_APP + 1;
const IDM_TRAY_CLOSE: usize = 1001;

static PREV_WNDPROC: AtomicIsize = AtomicIsize::new(0);

pub struct TrayIcon {
    hwnd: HWND,
    id: u32,
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
    owns_icon: bool,
}

impl TrayIcon {
    pub fn create(hwnd: HWND) -> Option<Self> {
        unsafe {
            let (icon, owns_icon) = load_tray_icon()
                .map(|icon| (icon, true))
                .or_else(|| LoadIconW(None, IDI_APPLICATION).ok().map(|icon| (icon, false)))?;

            let mut data = NOTIFYICONDATAW::default();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            data.uFlags = NIF_ICON | NIF_TIP | NIF_MESSAGE;
            data.uCallbackMessage = WM_TRAYICON;
            data.hIcon = icon;

            let tip = to_wide_null("agent-r");
            let tip_len = tip.len().min(data.szTip.len());
            data.szTip[..tip_len].copy_from_slice(&tip[..tip_len]);

            if Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                Some(Self {
                    hwnd,
                    id: 1,
                    icon,
                    owns_icon,
                })
            } else {
                if owns_icon {
                    let _ = DestroyIcon(icon);
                }
                None
            }
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let mut data = NOTIFYICONDATAW::default();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = self.hwnd;
            data.uID = self.id;
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);

            if self.owns_icon {
                let _ = DestroyIcon(self.icon);
            }
        }
    }
}

fn load_tray_icon() -> Option<windows::Win32::UI::WindowsAndMessaging::HICON> {
    let path = tray_icon_asset_path()?;
    let image = image::open(path).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }

    let mut bgra = image.into_raw();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let mask_stride = width.div_ceil(8);
    let mask = vec![0u8; (mask_stride * height) as usize];

    unsafe {
        let color_bitmap = CreateBitmap(
            width as i32,
            height as i32,
            1,
            32,
            Some(bgra.as_ptr() as *const _),
        );
        if color_bitmap.0 == 0 {
            return None;
        }

        let mask_bitmap = CreateBitmap(
            width as i32,
            height as i32,
            1,
            1,
            Some(mask.as_ptr() as *const _),
        );
        if mask_bitmap.0 == 0 {
            let _ = DeleteObject(color_bitmap);
            return None;
        }

        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
        };

        let icon = CreateIconIndirect(&icon_info).ok();

        let _ = DeleteObject(color_bitmap);
        let _ = DeleteObject(mask_bitmap);

        icon
    }
}

fn tray_icon_asset_path() -> Option<PathBuf> {
    let candidates = [
        Path::new("assets/tray.ico"),
        Path::new("assets/icon.ico"),
        Path::new("assets/wooper.png"),
        Path::new("assets/wooper.gif"),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(Path::to_path_buf)
}

pub fn install_tray_wndproc(hwnd: HWND) {
    if PREV_WNDPROC.load(Ordering::Relaxed) == 0 {
        unsafe {
            let prev = SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                tray_wnd_proc as *const () as usize as isize,
            );
            PREV_WNDPROC.store(prev, Ordering::Relaxed);
        }
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TRAYICON && ((lparam.0 as u32 == WM_RBUTTONUP) || (lparam.0 as u32 == WM_CONTEXTMENU)) {
        show_tray_menu(hwnd);
        return LRESULT(0);
    }

    if msg == WM_COMMAND {
        let command_id = (wparam.0 & 0xFFFF) as usize;
        if command_id == IDM_TRAY_CLOSE {
            unsafe {
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            return LRESULT(0);
        }
    }

    let prev_proc = PREV_WNDPROC.load(Ordering::Relaxed);
    if prev_proc != 0 {
        unsafe {
            let prev = std::mem::transmute(prev_proc);
            return CallWindowProcW(Some(prev), hwnd, msg, wparam, lparam);
        }
    }

    LRESULT(0)
}

fn show_tray_menu(hwnd: HWND) {
    let menu = unsafe {
        match CreatePopupMenu() {
            Ok(menu) => menu,
            Err(_) => return,
        }
    };

    let close_text = to_wide_null("Close");
    unsafe {
        let _ = AppendMenuW(menu, MF_STRING, IDM_TRAY_CLOSE, PCWSTR(close_text.as_ptr()));
    }

    let mut cursor = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut cursor);
        let _ = SetForegroundWindow(hwnd);

        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            None,
        );

        let _ = DestroyMenu(menu);
    }
}

fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
