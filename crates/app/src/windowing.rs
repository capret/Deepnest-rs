//! Native Windows frame and caption hit testing, adapted from ImagePro.
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
static MAXIMIZE_HOVERED: AtomicBool = AtomicBool::new(false);

pub fn maximize_hovered() -> bool {
    #[cfg(target_os = "windows")]
    {
        MAXIMIZE_HOVERED.load(Ordering::Relaxed)
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(target_os = "windows")]
pub fn platform_settings() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific {
        undecorated_shadow: true,
        corner_preference: iced::window::settings::platform::CornerPreference::Round,
        ..Default::default()
    }
}

#[cfg(not(target_os = "windows"))]
pub fn platform_settings() -> iced::window::settings::PlatformSpecific {
    Default::default()
}

#[cfg(target_os = "windows")]
pub fn install_native_titlebar(window: &dyn iced::window::Window) -> bool {
    use iced::window::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;

    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    let hwnd = handle.hwnd.get() as HWND;

    // Calling this again for the same procedure and id updates the existing
    // subclass instead of stacking another copy. This lets startup and later
    // window events both ensure the bridge is present.
    let subclassed =
        unsafe { SetWindowSubclass(hwnd, Some(titlebar_subclass), SUBCLASS_ID, 0) != 0 };
    subclassed && unsafe { configure_native_frame_once(hwnd) }
}

#[cfg(not(target_os = "windows"))]
pub fn install_native_titlebar(_window: &dyn iced::window::Window) -> bool {
    true
}

#[cfg(target_os = "windows")]
const SUBCLASS_ID: usize = 0x444E_5354;

#[cfg(target_os = "windows")]
const NATIVE_FRAME_PROP: [u16; 20] = [
    68, 101, 101, 112, 110, 101, 115, 116, 78, 97, 116, 105, 118, 101, 70, 114, 97, 109, 101, 0,
];

#[cfg(target_os = "windows")]
unsafe fn configure_native_frame_once(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetPropW, SetPropW, SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER,
    };

    if !unsafe { GetPropW(hwnd, NATIVE_FRAME_PROP.as_ptr()) }.is_null() {
        return true;
    }
    if unsafe { SetPropW(hwnd, NATIVE_FRAME_PROP.as_ptr(), 1usize as _) } == 0 {
        return false;
    }

    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        ) != 0
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn titlebar_subclass(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _subclass_id: usize,
    _reference_data: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, IsZoomed, RemovePropW, SendMessageW, HTMAXBUTTON, NCCALCSIZE_PARAMS,
        SC_MAXIMIZE, SC_RESTORE, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK,
        WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_SYSCOMMAND,
    };

    static MAXIMIZE_PRESSED: AtomicBool = AtomicBool::new(false);
    static TRACKING_NONCLIENT_MOUSE: AtomicBool = AtomicBool::new(false);

    if message == WM_NCCALCSIZE && wparam != 0 && unsafe { IsZoomed(hwnd) } == 0 {
        // Preserve Windows' real resize frame on restored windows, then reclaim
        // only the caption's top band for iced. This is the custom-frame shape
        // used by GPUI/Zed and keeps DWM's native caption-button hover path
        // alive; winit's fully client-extended frame does not. Do not replace
        // this with a child-HWND overlay: it was tested here and the restored
        // Snap Layout flyout still failed. Keep this native frame shape for restored Snap Layout hover.
        let params = lparam as *mut NCCALCSIZE_PARAMS;
        if !params.is_null() {
            let saved_top = unsafe { (*params).rgrc[0].top };
            let result = unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            unsafe {
                (*params).rgrc[0].top = saved_top;
            }
            return result;
        }
    }

    if message == WM_NCHITTEST {
        if unsafe { point_is_over_maximize(hwnd, lparam) } {
            // Windows uses this non-client result to expose the Windows 11
            // snap-layout flyout. Return it before the framework's default hit
            // test, matching Zed's native window procedure.
            return HTMAXBUTTON as isize;
        }
        if let Some(hit) = unsafe { resize_hit_test(hwnd, lparam) } {
            return hit as isize;
        }
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }

    if message == WM_NCMOUSEMOVE && wparam == HTMAXBUTTON as usize {
        if !TRACKING_NONCLIENT_MOUSE.swap(true, Ordering::Relaxed) {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                dwHoverTime: u32::MAX,
            };
            unsafe {
                TrackMouseEvent(&mut tracking);
            }
        }

        // Keep this as a real non-client hover sequence. Synthesizing a client
        // WM_MOUSEMOVE here makes winit replace non-client mouse tracking with
        // client tracking, which interrupts Windows' Snap Layout hover timer in
        // restored windows. GPUI dispatches this move directly to its UI; iced
        // has no equivalent hook, so the shared hover flag is read at draw time.
        unsafe { set_maximize_hovered(hwnd, true) };
    }

    if message == WM_NCMOUSELEAVE {
        TRACKING_NONCLIENT_MOUSE.store(false, Ordering::Relaxed);
        unsafe { set_maximize_hovered(hwnd, false) };
    }

    if matches!(message, WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK) && wparam == HTMAXBUTTON as usize {
        MAXIMIZE_PRESSED.store(true, Ordering::Relaxed);
        return 0;
    }

    let maximize_clicked = if message == WM_NCLBUTTONUP {
        MAXIMIZE_PRESSED.swap(false, Ordering::Relaxed) && wparam == HTMAXBUTTON as usize
    } else {
        false
    };
    if maximize_clicked {
        let command = if unsafe { IsZoomed(hwnd) } != 0 {
            SC_RESTORE
        } else {
            SC_MAXIMIZE
        };
        unsafe {
            SendMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0);
        }
        return 0;
    }

    if message == WM_NCDESTROY {
        MAXIMIZE_HOVERED.store(false, Ordering::Relaxed);
        unsafe {
            RemovePropW(hwnd, NATIVE_FRAME_PROP.as_ptr());
        }
        let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
        unsafe {
            RemoveWindowSubclass(hwnd, Some(titlebar_subclass), SUBCLASS_ID);
        }
        return result;
    }

    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(target_os = "windows")]
unsafe fn set_maximize_hovered(hwnd: windows_sys::Win32::Foundation::HWND, hovered: bool) {
    use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

    if MAXIMIZE_HOVERED.swap(hovered, Ordering::Relaxed) != hovered {
        unsafe {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn point_is_over_maximize(
    hwnd: windows_sys::Win32::Foundation::HWND,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> bool {
    use crate::tokens::layout as t;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut point = screen_point(lparam);
    let mut client = RECT::default();
    if unsafe { ScreenToClient(hwnd, &mut point) } == 0
        || unsafe { GetClientRect(hwnd, &mut client) } == 0
    {
        return false;
    }

    let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0;
    let control_width = (t::WINDOW_CONTROL_WIDTH * scale).round() as i32;
    let titlebar_height = (t::WINDOW_TITLEBAR_HEIGHT * scale).round() as i32;

    maximize_hit_test(
        point.x,
        point.y,
        client.right,
        control_width,
        titlebar_height,
    )
}

#[cfg(target_os = "windows")]
unsafe fn resize_hit_test(
    hwnd: windows_sys::Win32::Foundation::HWND,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> Option<u32> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, IsZoomed, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME,
    };

    if unsafe { IsZoomed(hwnd) } != 0 {
        return None;
    }

    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return None;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let border_x = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let border_y = unsafe { GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let point = screen_point(lparam);

    resize_hit_test_at(
        point.x,
        point.y,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        border_x,
        border_y,
    )
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
fn resize_hit_test_at(
    x: i32,
    y: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    border_x: i32,
    border_y: i32,
) -> Option<u32> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
    };

    let on_left = x < left + border_x;
    let on_right = x >= right - border_x;
    let on_top = y < top + border_y;
    let on_bottom = y >= bottom - border_y;

    match (on_left, on_right, on_top, on_bottom) {
        (true, _, true, _) => Some(HTTOPLEFT),
        (_, true, true, _) => Some(HTTOPRIGHT),
        (true, _, _, true) => Some(HTBOTTOMLEFT),
        (_, true, _, true) => Some(HTBOTTOMRIGHT),
        (true, _, _, _) => Some(HTLEFT),
        (_, true, _, _) => Some(HTRIGHT),
        (_, _, true, _) => Some(HTTOP),
        (_, _, _, true) => Some(HTBOTTOM),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn screen_point(
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::POINT {
    windows_sys::Win32::Foundation::POINT {
        x: lparam as i16 as i32,
        y: (lparam >> 16) as i16 as i32,
    }
}

#[cfg(target_os = "windows")]
fn maximize_hit_test(x: i32, y: i32, client_right: i32, width: i32, height: i32) -> bool {
    let close_left = client_right - width;
    let maximize_left = close_left - width;
    x >= maximize_left && x < close_left && y >= 0 && y < height
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{maximize_hit_test, resize_hit_test_at};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
    };

    #[test]
    fn maximize_hit_region_tracks_the_trailing_caption_controls_at_every_window_size() {
        assert!(maximize_hit_test(920, 24, 1_000, 46, 48));
        assert!(!maximize_hit_test(970, 24, 1_000, 46, 48));
        assert!(!maximize_hit_test(870, 24, 1_000, 46, 48));
        assert!(!maximize_hit_test(920, 48, 1_000, 46, 48));

        // Restored windows use the same trailing caption geometry, including
        // at the minimum supported width needed by Windows Snap Layouts.
        assert!(maximize_hit_test(420, 24, 500, 46, 48));
        assert!(!maximize_hit_test(470, 24, 500, 46, 48));
        assert!(!maximize_hit_test(370, 24, 500, 46, 48));
    }

    #[test]
    fn caption_hit_region_matches_shared_tokens_at_each_display_scale() {
        use crate::tokens::layout;

        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            let width = (layout::WINDOW_CONTROL_WIDTH * scale).round() as i32;
            let height = (layout::WINDOW_TITLEBAR_HEIGHT * scale).round() as i32;
            for logical_width in [500., 1000., 1360., 1920.] {
                let right = (logical_width * scale).round() as i32;
                let center = right - width - width / 2;
                assert!(maximize_hit_test(center, height / 2, right, width, height));
                assert!(!maximize_hit_test(
                    right - width / 2,
                    height / 2,
                    right,
                    width,
                    height
                ));
                assert!(!maximize_hit_test(
                    right - 3 * width,
                    height / 2,
                    right,
                    width,
                    height
                ));
                assert!(!maximize_hit_test(center, height, right, width, height));
            }
        }
    }

    #[test]
    fn every_restored_window_edge_and_corner_is_resizable() {
        let hit = |x, y| resize_hit_test_at(x, y, 100, 100, 900, 700, 8, 8);

        assert_eq!(hit(103, 103), Some(HTTOPLEFT));
        assert_eq!(hit(897, 103), Some(HTTOPRIGHT));
        assert_eq!(hit(103, 697), Some(HTBOTTOMLEFT));
        assert_eq!(hit(897, 697), Some(HTBOTTOMRIGHT));
        assert_eq!(hit(103, 400), Some(HTLEFT));
        assert_eq!(hit(897, 400), Some(HTRIGHT));
        assert_eq!(hit(500, 103), Some(HTTOP));
        assert_eq!(hit(500, 697), Some(HTBOTTOM));
        assert_eq!(hit(500, 400), None);
    }
}
