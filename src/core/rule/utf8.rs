//! Rule: write files as UTF-8.
pub const RULE: &str = concat!(
    "- Ghi mọi file bằng UTF-8 (không dùng PowerShell Set-Content/Out-File/>); ",
    "dùng công cụ sửa file hoặc [System.IO.File]::WriteAllText với UTF8."
);
