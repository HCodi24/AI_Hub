use super::*;

#[test]
fn plain_chats_use_separate_stable_runtime_folders_and_keep_empty_project() {
    let root = std::env::temp_dir().join(format!("ai-hub-plain-{}", uuid::Uuid::new_v4()));
    let config = root.join("config.json");
    let id = uuid::Uuid::new_v4().to_string();
    let folder = plain_chat_folder(&config, &id).unwrap();
    assert!(std::path::Path::new(&folder).is_dir());
    assert_eq!(folder, plain_chat_folder(&config, &id).unwrap());
    assert_ne!(
        folder,
        plain_chat_folder(&config, &uuid::Uuid::new_v4().to_string()).unwrap()
    );
    assert!(plain_chat_folder(&config, "../project").is_err());
    let request: ChatRequest = serde_json::from_value(serde_json::json!({
        "chat_id": id, "provider_id": "p", "model": "m", "folder": "",
        "messages": [{ "role": "user", "content": "Hello" }]
    }))
    .unwrap();
    assert!(check_request(&request).is_ok());
    let store = chat::ChatStore::new(root.join("chats"));
    store.save(&chat::new_record(&request)).unwrap();
    assert_eq!(store.get(&id).unwrap().unwrap().folder, "");
    assert_eq!(store.list().unwrap()[0].folder, "");
    std::fs::remove_dir_all(root).unwrap();
}
