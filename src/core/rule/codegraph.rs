//! Rule: read the CodeGraph index once at the start of a session, rebuild only when stale.
pub const RULE: &str = concat!(
    "Quy tắc CodeGraph (áp dụng cho yêu cầu code đầu tiên của phiên này):\n",
    "- Trước khi viết hay sửa code, đọc chỉ mục .codegraph của dự án.\n",
    "- Nếu dự án chưa có thư mục .codegraph: dừng lại và hỏi người dùng có muốn build .codegraph bằng MCP không, chờ họ trả lời.\n",
    "- Người dùng đồng ý: build .codegraph, đọc nó, rồi mới làm việc.\n",
    "- Người dùng không đồng ý: làm việc như bình thường, không hỏi lại.\n",
    "- Chỉ đọc .codegraph một lần ở đầu phiên, không đọc lại mỗi lượt.\n",
    "- Chỉ khi bạn quên nội dung của nó, hoặc code đã thay đổi nhiều: hỏi người dùng có muốn build lại .codegraph để đọc lại không. Đồng ý thì build và đọc lại, không đồng ý thì tiếp tục với những gì đã biết."
);
