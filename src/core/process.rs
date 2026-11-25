use std::{path::Path, process::Output, time::Duration};
use tokio::{process::Command, time::timeout};

pub fn configure_hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

#[cfg(windows)]
pub fn tie_to_app(child: &tokio::process::Child) {
    use std::{mem::size_of, ptr::null, sync::OnceLock};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };

    static JOB: OnceLock<Option<usize>> = OnceLock::new();
    let Some(process) = child.raw_handle() else {
        return;
    };
    let job = JOB.get_or_init(|| {
        // SAFETY: Các con trỏ null yêu cầu job vô danh với handle không kế thừa.
        let job = unsafe { CreateJobObjectW(null(), null()) };
        if job.is_null() {
            return None;
        }

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: Handle còn mở và bộ đệm có đúng kiểu và kích thước Win32 yêu cầu.
        let configured = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            // SAFETY: Handle vừa tạo thuộc quyền sở hữu của ứng dụng.
            unsafe { CloseHandle(job) };
            return None;
        }

        // Giữ handle suốt vòng đời ứng dụng để Windows dừng cây tiến trình con khi ứng dụng thoát.
        Some(job as usize)
    });
    if let Some(job) = *job {
        // SAFETY: Job giữ handle suốt vòng đời ứng dụng và Child vẫn sở hữu handle tiến trình.
        unsafe { AssignProcessToJobObject(job as HANDLE, process) };
    }
}

#[cfg(not(windows))]
pub fn tie_to_app(_child: &tokio::process::Child) {}

pub async fn run_with_timeout(
    command: &mut Command,
    duration: Duration,
) -> std::io::Result<std::process::Output> {
    match timeout(duration, command.output()).await {
        Ok(result) => result,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "process timed out",
        )),
    }
}

pub async fn git_output(repo: &Path, args: &[&str]) -> std::io::Result<Output> {
    let mut command = Command::new("git");
    command.args(args).current_dir(repo);
    configure_hidden(&mut command);
    run_with_timeout(&mut command, Duration::from_secs(30)).await
}

pub async fn kill_process_tree(pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(pid) = pid {
        let mut command = Command::new("taskkill");
        command.args(["/F", "/T", "/PID", &pid.to_string()]);
        configure_hidden(&mut command);
        let _ = command.status().await;
    }
    #[cfg(not(windows))]
    if let Some(pid) = pid {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
    }
}
