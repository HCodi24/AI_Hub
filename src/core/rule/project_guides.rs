//! Rule: the user switched off loading the project's own guide files, so the AI must not read them either.
pub const RULE: &str = "The user turned off project guide loading for this chat: do not read, open or follow AGENTS.md, GEMINI.md, CLAUDE.md, .agents/ or any other instruction or skill file of the project folder. Work only from this request and the code it names.";
