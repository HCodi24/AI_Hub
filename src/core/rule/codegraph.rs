//! Rule: read the CodeGraph index once at the start of a session, rebuild only when stale.
pub const RULE: &str = concat!(
    "CodeGraph rules (apply to the first coding request of this session):\n",
    "- Before writing or editing code, read the project's .codegraph index.\n",
    "- If the project has no .codegraph folder: stop and ask the user whether they want to build .codegraph with MCP, then wait for their answer.\n",
    "- The user agrees: build .codegraph, read it, and only then start working.\n",
    "- The user declines: work as usual, do not ask again.\n",
    "- Read .codegraph only once at the start of the session, do not re-read it on every turn.\n",
    "- Only if you forgot its content, or the code has changed a lot: ask the user whether they want to rebuild .codegraph and read it again. If they agree, build and read it again; if they decline, continue with what you already know."
);
