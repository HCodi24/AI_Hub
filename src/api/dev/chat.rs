//! Chat-folder selection and prompt preamble for dev mode.

pub(crate) fn effective_chat_folder(
    dev_mode: bool,
    dev_chat: bool,
    client_folder: &str,
    repo: &str,
) -> String {
    if dev_mode && dev_chat {
        repo.to_owned()
    } else {
        client_folder.to_owned()
    }
}

pub(crate) fn dev_chat_preamble(repo: &str) -> String {
    format!(
        concat!(
            "Bạn đang sửa chính ứng dụng MutiS (repo: {repo}) đang chạy ở chế độ dev.\n",
            "- Giao diện React ở web/src cần người dùng bấm 'Build giao diện' (trang tự tải lại ",
            "khi build xong); code Rust ở src/ cần 'Build & khởi động lại'.\n",
            "- Không chạy `cargo build --release`, không tắt tiến trình ai-hub, ",
            "không sửa target/.\n",
            "- Ghi file bằng UTF-8; giữ nguyên tính năng có sẵn; chạy `cargo test` ",
            "với CARGO_TARGET_DIR=target\\dev-check nếu cần.\n",
            "- Cuối cùng báo: đã sửa file nào, có cần Build & khởi động lại không.\n{}"
        ),
        crate::core::rule::english_rules::RULE,
        repo = repo
    )
}

#[cfg(test)]
mod tests {
    use super::effective_chat_folder;

    #[test]
    fn english_rule_authoring_is_scoped_to_app_development() {
        let rule = crate::core::rule::english_rules::RULE;
        assert!(super::dev_chat_preamble("C:/repo").contains(rule));
        assert!(!crate::core::rule::worker_defaults().contains(rule));
        for indexed in [true, false] {
            assert!(!crate::core::rule::first_request_rules(indexed).contains(rule));
        }
    }

    #[test]
    fn dev_chat_folder_uses_repo_only_when_dev_mode_is_on() {
        assert_eq!(
            effective_chat_folder(true, true, "C:/client", "C:/repo"),
            "C:/repo"
        );
        assert_eq!(
            effective_chat_folder(true, false, "C:/client", "C:/repo"),
            "C:/client"
        );
        assert_eq!(
            effective_chat_folder(false, true, "C:/saved", "C:/repo"),
            "C:/saved"
        );
        assert_eq!(
            effective_chat_folder(false, false, "C:/client", "C:/repo"),
            "C:/client"
        );
    }
}
