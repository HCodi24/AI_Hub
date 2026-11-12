//! Rule: write files as UTF-8.
pub const RULE: &str = concat!(
    "- Write every file as UTF-8 (do not use PowerShell Set-Content/Out-File/>); ",
    "use the file editing tool or [System.IO.File]::WriteAllText with UTF8."
);
