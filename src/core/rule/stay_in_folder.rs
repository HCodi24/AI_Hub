//! Rule: read only inside the project folder the session started in. A prompt rule only: Codex's Windows sandbox cannot
//! restrict reads to a folder, so the app cannot enforce it.
pub const RULE: &str = concat!(
    "Folder scope:\n",
    "- Work only inside the project folder this session started in, plus folders the user names. Do not search, list or read ",
    "other folders or repositories to work out what the user means.\n",
    "- If what the user asks about is not in this folder, say so in one sentence and ask which folder to use. ",
    "Never present a guess about another folder as if the user had confirmed it.\n",
    "- If you need to read outside the folder, ask the user first and wait for the answer."
);
