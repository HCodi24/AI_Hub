//! Rule: read the CodeGraph index once at the start of a session, rebuild only when stale.
pub const RULE: &str = concat!(
    "CodeGraph rules (apply to the first coding request of this session):\n",
    "- Before writing or editing code, read the project's .codegraph index by calling the codegraph_explore tool exactly once, in your very first step, with one question that describes the job. Do not list the .codegraph folder, do not list the project tree, and do not first check which tools exist (the status line below says whether the index exists).\n",
    "- If the project has no .codegraph folder: stop and ask the user whether they want to build .codegraph with MCP, then wait for their answer.\n",
    "- The user agrees: build .codegraph, read it, and only then start working.\n",
    "- The user declines: work as usual, do not ask again.\n",
    "- Read .codegraph only once at the start of the session, do not re-read it on every turn.\n",
    "- The app syncs the index itself before this request; the status line says whether that worked. Never ask the user to rebuild because the code changed. Only if the status says the sync failed and the results look wrong, tell the user the index may be out of date."
);
