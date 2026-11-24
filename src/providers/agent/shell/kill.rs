#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use tokio::process::Command;

pub(super) async fn kill_process_tree(pid: Option<u32>) {
    crate::core::process::kill_process_tree(pid).await;
    #[cfg(windows)]
    if let Some(pid) = pid {
        terminate_windows_process_tree(pid);
    }
    #[cfg(unix)]
    if let Some(pid) = pid {
        let mut command = Command::new("kill");
        command
            .args(["-TERM", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::core::process::configure_hidden(&mut command);
        let _ = command.status().await;
    }
}

#[cfg(windows)]
fn terminate_windows_process_tree(root: u32) {
    use std::collections::HashSet;
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        thread_count: u32,
        parent_process_id: u32,
        base_priority: i32,
        flags: u32,
        executable: [u16; 260],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
        fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> *mut c_void;
        fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(0x00000002, 0) };
    if snapshot.is_null() || snapshot as isize == -1 {
        return;
    }
    let mut processes = Vec::new();
    unsafe {
        let mut entry = std::mem::zeroed::<ProcessEntry32W>();
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                processes.push((entry.process_id, entry.parent_process_id));
                entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    let mut seen = HashSet::from([root]);
    let mut pending = vec![root];
    let mut tree = Vec::new();
    while let Some(parent) = pending.pop() {
        tree.push(parent);
        for (process, process_parent) in &processes {
            if *process_parent == parent && seen.insert(*process) {
                pending.push(*process);
            }
        }
    }
    for process in tree.into_iter().rev() {
        unsafe {
            let handle = OpenProcess(0x0001, 0, process);
            if !handle.is_null() {
                TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }
    }
}
