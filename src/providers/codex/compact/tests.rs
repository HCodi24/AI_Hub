use super::*;

#[test]
fn a_thread_is_compacted_only_once_it_is_over_the_budget() {
    note_size("t-small", 90_000);
    note_size("t-big", 160_000);
    assert_eq!(needs_compaction("t-small", 150_000), None);
    assert_eq!(needs_compaction("t-big", 150_000), Some(160_000));
    assert_eq!(needs_compaction("t-big", 0), None, "0 = off");
    assert_eq!(needs_compaction("t-unknown", 150_000), None, "a thread the app has not measured is left alone");
    // A later, smaller reading (after the thread was compacted) replaces the old one.
    note_size("t-big", 40_000);
    assert_eq!(needs_compaction("t-big", 150_000), None);
}

#[test]
fn the_budget_comes_from_the_config_file() {
    let dir = std::env::temp_dir().join(format!("mutis-compact-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    assert_eq!(thread_token_budget(&path), 0, "no file: compaction stays off");
    std::fs::write(&path, br#"{"thread_token_budget":123000}"#).unwrap();
    assert_eq!(thread_token_budget(&path), 123_000);
    std::fs::write(&path, b"not json").unwrap();
    assert_eq!(thread_token_budget(&path), 0, "unreadable file: compaction stays off");
    std::fs::remove_dir_all(&dir).ok();
}

async fn turn(client: &mut Client, id: &str, text: &str) -> String {
    client.call("turn/start", json!({"threadId":id,"effort":"low","input":[{"type":"text","text":text}]})).await.unwrap();
    let mut reply = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    loop {
        let value = tokio::time::timeout_at(deadline, client.next()).await.expect("took too long").unwrap();
        if value["method"] == "item/completed" && value["params"]["item"]["type"] == "agentMessage" {
            reply = value["params"]["item"]["text"].as_str().unwrap_or("").to_owned();
        }
        if value["method"] == "turn/completed" { return reply; }
    }
}

/// Real Codex: a second client resumes the thread (as the app does on the next message), the app's `compact` runs, and the
/// thread still knows a fact from before and a rule from the first message. Uses the app's signed-in Codex home.
#[tokio::test]
#[ignore = "Needs MUTIS_CODEX_TEST_EXE and the app's signed-in Codex home; runs real turns"]
async fn live_a_resumed_thread_keeps_its_memory_after_compaction() {
    let exe = std::env::var_os("MUTIS_CODEX_TEST_EXE").expect("Set MUTIS_CODEX_TEST_EXE");
    let home = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("ai-hub").join("codex").join("codex-cli");
    let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let params = json!({"cwd":folder,"model":"gpt-6.2-sol","approvalPolicy":"untrusted","sandbox":"read-only","selectedCapabilityRoots":[],
        "developerInstructions":"Rule R1: end every reply with the word BANANA."});
    let mut first = Client::start(Path::new(&exe), &home, &folder).await.unwrap();
    let thread = first.call("thread/start", params.clone()).await.unwrap();
    let id = thread["thread"]["id"].as_str().unwrap().to_owned();
    turn(&mut first, &id, "MutiS rule R2 (for the whole session): put the word MANGO right before BANANA. Write 150 words on why the sky is blue. Remember the secret code 4417.").await;
    first.close().await;

    // What the "Nén ngữ cảnh" button runs, in a client of its own.
    compact_saved(Path::new(&exe), &home, &folder, &id).await.unwrap();
    let mut second = Client::start(Path::new(&exe), &home, &folder).await.unwrap();
    let mut resume = params;
    resume["threadId"] = json!(id);
    second.call("thread/resume", resume).await.unwrap();
    let reply = turn(&mut second, &id, "What was the secret code? Answer in one line, following rules R1 and R2.").await;
    second.close().await;
    assert!(reply.contains("4417"), "the fact was lost: {reply}");
    assert!(reply.trim_end().ends_with("MANGO BANANA"), "the rules were lost: {reply}");
}
