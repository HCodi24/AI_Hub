use crate::api::{ApiError, ApiResult};
use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

/// Native "choose folder" dialog on the machine running the app. `{"path": null}` when the user cancels.
/// shortcut: Windows uses the native Open dialog in folder-picking mode; other platforms remain unsupported.
pub(crate) async fn pick_folder() -> ApiResult<Json<Value>> {
    #[cfg(windows)]
    {
        let path = tokio::task::spawn_blocking(pick_folder_windows)
            .await
            .map_err(|_| folder_dialog_error())?
            .map_err(|_| folder_dialog_error())?;
        Ok(Json(json!({ "path": path })))
    }
    #[cfg(not(windows))]
    Err(ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "Chỉ hỗ trợ trên Windows",
    ))
}

#[cfg(windows)]
fn folder_dialog_error() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Không mở được hộp thoại chọn thư mục",
    )
}

#[cfg(windows)]
fn pick_folder_windows() -> Result<Option<String>, ()> {
    use std::{ffi::c_void, ptr};
    use windows_sys::{
        core::GUID,
        Win32::{
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            UI::{
                HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
                Shell::{FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH},
                WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, ShowWindow, SW_SHOWNOACTIVATE,
                    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
                },
            },
        },
    };

    const CLSID_FILE_OPEN_DIALOG: GUID = GUID {
        data1: 0xdc1c5a9c,
        data2: 0xe88a,
        data3: 0x4dde,
        data4: [0xa5, 0xa1, 0x60, 0xf8, 0x2a, 0x20, 0xae, 0xf7],
    };
    const IID_FILE_OPEN_DIALOG: GUID = GUID {
        data1: 0xd57c7288,
        data2: 0xd4ad,
        data3: 0x4768,
        data4: [0xbe, 0x02, 0x9d, 0x96, 0x95, 0x32, 0xd9, 0x60],
    };
    const HRESULT_CANCELLED: i32 = 0x8007_04c7u32 as i32;
    // IFileDialog inherits IModalWindow; these are its stable COM vtable slots.
    const SHOW: usize = 3;
    const SET_OPTIONS: usize = 9;
    const GET_OPTIONS: usize = 10;
    const GET_RESULT: usize = 20;
    // IShellItem inherits IUnknown; GetDisplayName is its fourth method.
    const GET_DISPLAY_NAME: usize = 5;

    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    // Retain the thread's previous DPI context until all COM and HWND resources are gone.
    struct ThreadDpiAwarenessContext(Option<windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT>);
    impl ThreadDpiAwarenessContext {
        fn per_monitor_v2() -> Self {
            let previous =
                unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
            Self((!previous.is_null()).then_some(previous))
        }
    }
    impl Drop for ThreadDpiAwarenessContext {
        fn drop(&mut self) {
            if let Some(previous) = self.0 {
                unsafe { SetThreadDpiAwarenessContext(previous) };
            }
        }
    }

    struct TemporaryOwnerWindow(windows_sys::Win32::Foundation::HWND);
    impl TemporaryOwnerWindow {
        fn create() -> Result<Self, ()> {
            const STATIC_CLASS: [u16; 7] = [83, 84, 65, 84, 73, 67, 0];
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    STATIC_CLASS.as_ptr(),
                    ptr::null(),
                    WS_POPUP,
                    -32_000,
                    -32_000,
                    1,
                    1,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null(),
                )
            };
            if hwnd.is_null() {
                return Err(());
            }
            unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
            Ok(Self(hwnd))
        }
    }
    impl Drop for TemporaryOwnerWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.0) };
        }
    }

    struct ComObject(*mut c_void);
    impl Drop for ComObject {
        fn drop(&mut self) {
            unsafe {
                let vtable = *(self.0.cast::<*const *const c_void>());
                let release: unsafe extern "system" fn(*mut c_void) -> u32 =
                    std::mem::transmute(*vtable.add(2));
                release(self.0);
            }
        }
    }

    unsafe fn method<T: Copy>(object: *mut c_void, index: usize) -> T {
        let vtable = *(object.cast::<*const *const c_void>());
        std::mem::transmute_copy(&*vtable.add(index))
    }

    let _dpi_context = ThreadDpiAwarenessContext::per_monitor_v2();
    let initialized = unsafe {
        CoInitializeEx(
            ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    if initialized < 0 {
        return Err(());
    }
    let _apartment = ComApartment;
    let owner = TemporaryOwnerWindow::create()?;

    unsafe {
        let mut raw_dialog: *mut c_void = ptr::null_mut();
        let result = CoCreateInstance(
            &CLSID_FILE_OPEN_DIALOG,
            ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_FILE_OPEN_DIALOG,
            &mut raw_dialog,
        );
        if result < 0 || raw_dialog.is_null() {
            return Err(());
        }
        let dialog = ComObject(raw_dialog);

        let mut options = 0u32;
        let get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32 =
            method(dialog.0, GET_OPTIONS);
        if get_options(dialog.0, &mut options) < 0 {
            return Err(());
        }
        let set_options: unsafe extern "system" fn(*mut c_void, u32) -> i32 =
            method(dialog.0, SET_OPTIONS);
        if set_options(dialog.0, options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM) < 0 {
            return Err(());
        }
        let show: unsafe extern "system" fn(
            *mut c_void,
            windows_sys::Win32::Foundation::HWND,
        ) -> i32 = method(dialog.0, SHOW);
        let shown = show(dialog.0, owner.0);
        if shown == HRESULT_CANCELLED {
            return Ok(None);
        }
        if shown < 0 {
            return Err(());
        }

        let get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
            method(dialog.0, GET_RESULT);
        let mut raw_item = ptr::null_mut();
        if get_result(dialog.0, &mut raw_item) < 0 || raw_item.is_null() {
            return Err(());
        }
        let item = ComObject(raw_item);
        let get_display_name: unsafe extern "system" fn(
            *mut c_void,
            windows_sys::Win32::UI::Shell::SIGDN,
            *mut *mut u16,
        ) -> i32 = method(item.0, GET_DISPLAY_NAME);
        let mut raw_path: *mut u16 = ptr::null_mut();
        if get_display_name(item.0, SIGDN_FILESYSPATH, &mut raw_path) < 0 || raw_path.is_null() {
            return Err(());
        }
        let mut length = 0usize;
        while *raw_path.add(length) != 0 {
            length += 1;
        }
        let wide_path = std::slice::from_raw_parts(raw_path, length);
        let wide_path = wide_path.to_vec();
        CoTaskMemFree(raw_path.cast());
        let path = String::from_utf16(&wide_path).map_err(|_| ())?;
        Ok(Some(path))
    }
}
