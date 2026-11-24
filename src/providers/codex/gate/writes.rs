/// Commands that only look. A Read-mode command outside this list is put to the user, so a way of writing nobody listed cannot slip through.
const READ_ONLY: [&str; 38] = ["get-childitem", "gci", "ls", "dir", "get-content", "gc", "cat", "type", "get-item", "gi", "test-path", "resolve-path",
    "get-location", "pwd", "get-command", "where", "where.exe", "select-string", "sls", "rg", "grep", "findstr", "head", "tail", "wc", "sort", "uniq",
    "measure-object", "select-object", "where-object", "sort-object", "foreach-object", "format-table", "out-string", "get-filehash", "echo", "write-output", "git"];
const READ_ONLY_GIT: [&str; 9] = ["status", "log", "diff", "show", "branch", "ls-files", "rev-parse", "blame", "grep"];

/// The script inside `powershell -Command "..."` or `sh -c "..."`; the command itself when it is not wrapped.
fn inner(command: &str) -> &str {
    let lower = command.to_lowercase();
    ["-command ", " -lc ", " -c "].iter()
        .filter_map(|flag| lower.find(flag).map(|at| at + flag.len()))
        .min()
        .map_or(command, |at| command[at..].trim().trim_matches(|c| matches!(c, '"' | '\'')))
}

/// True when the command may write: a redirect, a known writing word, or any step that is not a known read-only command.
pub(crate) fn writes(command: &str) -> bool {
    let c = inner(command).to_lowercase().replace("2>&1", "").replace("2>$null", "").replace(">$null", "");
    const WORDS: [&str; 27] = ["writealltext", "writeallbytes", "appendalltext", "set-content", "add-content", "out-file", "new-item", "remove-item", "rename-item", "move-item", "copy-item",
        "clear-content", "tee-object", "mkdir", "rmdir", "del ", "rm ", "mv ", "cp ", "touch ", "tee ", "sed -i", "git add", "git commit",
        "git checkout", "npm install", "pip install"];
    if c.contains('>') || WORDS.iter().any(|word| c.contains(word)) { return true; }
    c.split(|ch| matches!(ch, ';' | '|' | '&' | '\n')).map(str::trim).filter(|step| !step.is_empty()).any(|step| {
        let mut words = step.trim_start_matches(|ch| matches!(ch, '(' | '$' | '"' | '\'')).split_whitespace();
        let first = words.next().unwrap_or("").trim_end_matches(|ch| matches!(ch, '"' | '\'' | ')'));
        let first = first.rsplit(['\\', '/']).next().unwrap_or(first);
        let first = first.strip_suffix(".exe").unwrap_or(first);
        !READ_ONLY.contains(&first) || (first == "git" && !words.next().is_some_and(|sub| READ_ONLY_GIT.contains(&sub)))
    })
}
