use super::parse::{last_report, parse_report};
use super::store::{chat_totals, history_path};
use super::*;
use crate::features::chat::{ChatMessage, ChatRecord};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn temp_dir() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("mutis-token-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn cleanup(directory: &Path) {
    let _ = fs::remove_dir_all(directory);
}

fn assistant(model: &str, logs: &[&str]) -> serde_json::Value {
    serde_json::json!({"role": "assistant", "content": "reply", "model": model, "logs": logs})
}

fn chat(id: &str, messages: Vec<serde_json::Value>) -> ChatRecord {
    serde_json::from_value(serde_json::json!({
        "id": id, "title": "token test", "provider_id": "codex", "model": "test",
        "folder": "", "updated_at": 0, "messages": messages
    }))
    .unwrap()
}

fn usage(model: &str, input: u64, output: u64) -> ModelUsage {
    ModelUsage {
        model: model.to_owned(),
        input,
        output,
        total: input + output,
    }
}

fn row(at: u64, models: Vec<ModelUsage>) -> String {
    format!("{}\n", serde_json::to_string(&TokenSnapshot { at, models }).unwrap())
}

fn write_rows(directory: &Path, id: &str, rows: &str) -> PathBuf {
    let path = history_path(directory, id).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, rows).unwrap();
    path
}

#[test]
fn parses_the_frontend_report_grammar() {
    for log in [
        "Token: vào 1200 (cache 300), ra 45 (suy luận 10)",
        "  Token: vào 1200, ra 45  ",
        "Token: vào 1200, ra 45, chi phí $0.12",
        "Token: vào 1200, ra 45, chi phí 0,12",
        "token: vào 0, ra 0",
        "TOKEN: VÀO 7, RA 3",
        "Token: vào 7, ra 3, CHI PHÍ $1.5",
        // `\s*` after the comma, so no space is required before `ra`.
        "Token: vào 1,ra 2",
        // `\$?\s*`, so whitespace between the dollar sign and the amount is allowed.
        "Token: vào 1, ra 2, chi phí $ 0.12",
        // The cost tail is `[\d.,]+`, so a trailing comma is part of the amount.
        "Token: vào 1, ra 2, chi phí $1.2,",
    ] {
        assert!(parse_report(log).is_some(), "should accept {log:?}");
    }
    assert_eq!(parse_report("Token: vào 1200 (cache 300), ra 45 (suy luận 10)"), Some((1200, 45)));
    assert_eq!(parse_report("TOKEN: VÀO 7, RA 3"), Some((7, 3)));
    assert_eq!(parse_report("Token: vào 0, ra 0"), Some((0, 0)));
    assert_eq!(parse_report("Token: vào 1,ra 2"), Some((1, 2)));
    assert_eq!(parse_report("Token: vào 1, ra 2, chi phí $ 0.12"), Some((1, 2)));
    assert_eq!(parse_report("Token: vào 1, ra 2, chi phí $1.2,"), Some((1, 2)));
}

#[test]
fn rejects_logs_the_frontend_rejects() {
    for log in [
        "Token: vào1, ra2",
        "Token: vào 1, ra2",
        "Token: vào1, ra 2",
        "Token: vào 1, ra 2, chi phí $",
        "Token: vào 1, ra 2, chi phí",
        "Token: vào 1, ra 2, giá $1",
        "Token: vào 1, ra 2, chi phí $1.2 extra",
        "Token: vào 1, ra 2, chi phí $x",
        "Token vào 1, ra 2",
        "Token: vào 1 ra 2",
        "Token: vào 1, ra 2 (",
        "Token: vào 9007199254740992, ra 1",
        "Read file",
        "",
    ] {
        assert!(parse_report(log).is_none(), "should reject {log:?}");
    }
}

#[test]
fn last_valid_report_wins_and_duplicates_are_not_summed() {
    let logs = vec![
        "Token: vào 10, ra 1".to_owned(),
        "Read file".to_owned(),
        "Token: vào 20, ra 2".to_owned(),
        "Token: vào 30, ra 3".to_owned(),
    ];
    assert_eq!(last_report(&logs), Some((30, 3)));
    let messages: Vec<ChatMessage> = serde_json::from_value(serde_json::json!([
        assistant("m", &["Token: vào 10, ra 1", "Token: vào 20, ra 2"]),
        assistant("m", &["Token: vào 5, ra 5"]),
        {"role": "user", "content": "hi", "logs": ["Token: vào 99, ra 99"]}
    ]))
    .unwrap();
    assert_eq!(chat_totals(&messages).unwrap(), vec![usage("m", 25, 7)]);
}

#[test]
fn totals_keep_zero_and_unknown_models_in_canonical_order() {
    let messages: Vec<ChatMessage> = serde_json::from_value(serde_json::json!([
        assistant("zeta", &["Token: vào 1, ra 1"]),
        assistant("", &["Token: vào 0, ra 0"]),
        assistant("alpha", &["Token: vào 9, ra 9"]),
        assistant("zeta", &["Token: vào 2, ra 2"])
    ]))
    .unwrap();
    assert_eq!(
        chat_totals(&messages).unwrap(),
        vec![usage("", 0, 0), usage("alpha", 9, 9), usage("zeta", 3, 3)],
        "canonical order is the exact model identity, not the total"
    );
}

#[test]
fn record_dedupes_identical_totals_across_restarts() {
    let directory = temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    let record_chat = chat(&id, vec![assistant("m", &["Token: vào 10, ra 5"])]);
    let first = record(&directory, &record_chat).unwrap().unwrap();
    assert_eq!(first.models, vec![usage("m", 10, 5)]);
    // A fresh call is the restart case: the file, not memory, decides.
    assert!(record(&directory, &record_chat).unwrap().is_none());
    assert_eq!(history(&directory, &id).unwrap().len(), 1);
    cleanup(&directory);
}

#[test]
fn corrected_reports_append_and_timestamps_stay_increasing() {
    let directory = temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    let first = chat(&id, vec![assistant("m", &["Token: vào 10, ra 5"])]);
    let second = chat(&id, vec![assistant("m", &["Token: vào 10, ra 5", "Token: vào 12, ra 6"])]);
    let one = record(&directory, &first).unwrap().unwrap();
    let two = record(&directory, &second).unwrap().unwrap();
    assert!(two.at > one.at, "a new observation never reuses the previous timestamp");
    let rows = history(&directory, &id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].models, vec![usage("m", 12, 6)]);
    cleanup(&directory);
}

#[test]
fn each_chat_keeps_its_own_file() {
    let directory = temp_dir();
    let first = uuid::Uuid::new_v4().to_string();
    let second = uuid::Uuid::new_v4().to_string();
    record(&directory, &chat(&first, vec![assistant("a", &["Token: vào 1, ra 1"])])).unwrap();
    record(&directory, &chat(&second, vec![assistant("b", &["Token: vào 2, ra 2"])])).unwrap();
    assert_eq!(history(&directory, &first).unwrap()[0].models, vec![usage("a", 1, 1)]);
    assert_eq!(history(&directory, &second).unwrap()[0].models, vec![usage("b", 2, 2)]);
    assert!(history(&directory, &uuid::Uuid::new_v4().to_string()).unwrap().is_empty());
    cleanup(&directory);
}

#[test]
fn latest_points_are_returned_while_the_file_keeps_every_row() {
    let directory = temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    let mut rows = String::new();
    for index in 0..(MAX_POINTS + 5) {
        rows.push_str(&row(index as u64 + 1, vec![usage("m", index as u64, 0)]));
    }
    let path = write_rows(&directory, &id, &rows);
    let points = history(&directory, &id).unwrap();
    assert_eq!(points.len(), MAX_POINTS);
    assert_eq!(points[0].at, 6);
    assert_eq!(points[MAX_POINTS - 1].at, (MAX_POINTS + 5) as u64);
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), MAX_POINTS + 5);
    cleanup(&directory);
}

#[test]
fn corrupt_complete_rows_are_rejected() {
    let directory = temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    for rows in [
        "not json\n".to_owned(),
        row(1, Vec::new()),
        row(1, vec![usage("m", 1, 1), usage("m", 2, 2)]),
        row(1, vec![ModelUsage { model: "m".into(), input: 1, output: 1, total: 3 }]),
        row(JS_DATE_MAX + 1, vec![usage("m", 1, 1)]),
        format!("{}{}", row(2, vec![usage("m", 1, 1)]), row(2, vec![usage("m", 2, 2)])),
        format!("{}{}", row(3, vec![usage("m", 1, 1)]), row(2, vec![usage("m", 2, 2)])),
    ] {
        write_rows(&directory, &id, &rows);
        assert!(history(&directory, &id).is_err(), "should reject {rows:?}");
    }
    cleanup(&directory);
}

#[test]
fn invalid_ids_are_rejected_without_touching_disk() {
    let directory = temp_dir();
    assert!(history(&directory, "not-a-uuid").is_err());
    assert!(history(&directory, "../escape").is_err());
    let mut record_chat = chat(&uuid::Uuid::new_v4().to_string(), vec![assistant("m", &["Token: vào 1, ra 1"])]);
    record_chat.id = "not-a-uuid".to_owned();
    assert!(record(&directory, &record_chat).is_err());
    assert!(!directory.join("token-usage").exists());
    cleanup(&directory);
}

#[test]
fn partial_tail_is_ignored_on_read_and_repaired_on_write() {
    let directory = temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    let complete = row(1, vec![usage("m", 1, 1)]);
    let path = write_rows(&directory, &id, &format!("{complete}{{\"at\":2,\"models\":["));
    assert_eq!(history(&directory, &id).unwrap().len(), 1, "an unfinished row is not a row");
    let next = record(&directory, &chat(&id, vec![assistant("m", &["Token: vào 4, ra 4"])]))
        .unwrap()
        .unwrap();
    assert!(next.at > 1);
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 2, "the unfinished tail is dropped before appending");
    assert_eq!(history(&directory, &id).unwrap().len(), 2);
    cleanup(&directory);
}

#[test]
fn reads_never_write_and_missing_history_stays_empty() {
    let directory = temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    assert!(history(&directory, &id).unwrap().is_empty());
    assert!(!directory.join("token-usage").exists(), "a read must not create files");
    cleanup(&directory);
}
