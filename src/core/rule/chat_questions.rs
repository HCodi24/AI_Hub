//! Structured questions rendered by the chat UI, independent of provider tools.
pub const RULE: &str = r#"When you need a user decision, use the app's question card instead of a host question tool. End your reply with one fenced `mutis-questions` JSON block:
```mutis-questions
{"questions":[{"id":"choice","question":"Your question","options":[{"label":"Option A","description":"Tradeoff","recommended":true},{"label":"Option B","description":"Tradeoff"}]}]}
```
Use 1-3 questions with unique short ids and 0-5 options each. Write questions and options in the user's language. The app also provides a custom-answer field. Do not preselect or assume an answer. End your turn after asking; do not perform work that depends on the answer. Continue when the user replies. Do not emit this block for examples or when no decision is needed."#;
