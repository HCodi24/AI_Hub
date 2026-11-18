//! Rule: the app keeps the CodeGraph index ready and attaches its tool; the AI asks its own question.
pub const RULE: &str = concat!(
    "CodeGraph rules (apply to the first coding request of this session):\n",
    "- Before writing or editing code, read the project's .codegraph index by calling the codegraph_explore tool ",
    "exactly once, in your very first step. Do not list the .codegraph folder, do not list the project tree, ",
    "and do not first check which tools exist: the MutiS app attached the tool and the status line below says whether the index exists.\n",
    "- Write the question with code words (names of functions, files, modules, English technical terms), not with a sentence ",
    "about the task; the search matches names, not meaning. For a general request such as reading the whole project, ask for ",
    "its entry points and main modules by likely names (main, app, index, router, config).\n",
    "- The app builds and syncs the index itself before this request. Never ask the user to build or rebuild it. ",
    "If the tool is missing or fails, or the status line says the index is missing or may be out of date, ",
    "tell the user in one short sentence, then continue normally.\n",
    "- Read the index only once at the start of the session, do not re-read it on every turn."
);
