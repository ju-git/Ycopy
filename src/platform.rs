//! Thin Win32 helpers (the only `unsafe` code outside the GUI toolkit).

use windows_sys::Win32::Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    SetStdHandle,
};

/// Size of the client area of a window, in pixels.
pub fn client_size(hwnd: *mut std::ffi::c_void) -> Option<(i32, i32)> {
    let mut rect = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: `rect` is a valid out-pointer; an invalid HWND just makes the call fail.
    let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) };
    (ok != 0).then_some((rect.right - rect.left, rect.bottom - rect.top))
}

/// Raises a child window above its siblings and stops siblings (e.g. the status
/// bar it overlaps) from painting over it.
pub fn raise_child(hwnd: *mut std::ffi::c_void) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW,
        SetWindowPos, WS_CLIPSIBLINGS,
    };
    // SAFETY: plain Win32 calls; an invalid HWND just makes them fail.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style | WS_CLIPSIBLINGS as isize);
        SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

/// Where the mouse cursor is over a report-style list view: `Some((row, column))`
/// on a row, `None` on the empty area. (Hit-tests the cursor position itself,
/// because the control's own click notification is not reliable on blank space.)
pub fn list_view_hit_test(hwnd: *mut std::ffi::c_void) -> Option<(usize, usize)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, SendMessageW};

    const LVM_SUBITEMHITTEST: u32 = 0x1000 + 57;
    #[repr(C)]
    struct HitTestInfo {
        pt: POINT,
        flags: u32,
        item: i32,
        sub_item: i32,
        group: i32,
    }

    let mut pt = POINT { x: 0, y: 0 };
    // SAFETY: `pt` and `info` are valid for the duration of the calls; an invalid HWND just makes them fail.
    unsafe {
        if GetCursorPos(&mut pt) == 0 || ScreenToClient(hwnd, &mut pt) == 0 {
            return None;
        }
        let mut info = HitTestInfo { pt, flags: 0, item: -1, sub_item: 0, group: 0 };
        SendMessageW(hwnd, LVM_SUBITEMHITTEST, 0, &mut info as *mut HitTestInfo as isize);
        (info.item >= 0 && info.sub_item >= 0).then_some((info.item as usize, info.sub_item as usize))
    }
}

/// The user's locale name, e.g. `fr-FR`.
pub fn user_locale() -> Option<String> {
    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
    // SAFETY: the buffer is valid for `buf.len()` UTF-16 units.
    let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..(n - 1) as usize]))
}

/// The executable uses the GUI subsystem, so it starts without a console.
/// In command-line mode, attach to the parent console (if any) and make the
/// standard output/error streams usable, unless they were redirected.
pub fn attach_parent_console() {
    // SAFETY: plain Win32 calls with valid arguments.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return; // no parent console (e.g. launched from Explorer)
        }
        let conout: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        for std in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let current = GetStdHandle(std);
            if current.is_null() || current == INVALID_HANDLE_VALUE {
                let h = CreateFileW(
                    conout.as_ptr(),
                    GENERIC_WRITE,
                    FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                );
                if h != INVALID_HANDLE_VALUE {
                    SetStdHandle(std, h);
                }
            }
        }
    }
}
