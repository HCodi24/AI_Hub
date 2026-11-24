use super::*;
#[tokio::test]
#[ignore = "Requires MUTIS_CODEX_TEST_EXE; uses a disposable home and no inference"]
async fn installed_app_server_uses_an_isolated_home() {
    let executable =
        std::env::var_os("MUTIS_CODEX_TEST_EXE").expect("Set MUTIS_CODEX_TEST_EXE");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/codex-live-test")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).unwrap();
    let mut client = Client::start(Path::new(&executable), &root, &root)
        .await
        .unwrap();
    let account = client
        .call("account/read", json!({"refreshToken":false}))
        .await
        .unwrap();
    assert!(
        account["account"].is_null(),
        "New MutiS home must not inherit personal authentication"
    );
    let thread = client.call("thread/start",json!({"cwd":root,"model":"gpt-6.1-sol","sandbox":"read-only","approvalPolicy":"never","selectedCapabilityRoots":[]})).await.unwrap();
    assert!(thread["thread"]["id"].as_str().is_some());
    let login = client
        .call("account/login/start", json!({"type":"chatgpt"}))
        .await
        .unwrap();
    let url = reqwest::Url::parse(login["authUrl"].as_str().unwrap()).unwrap();
    assert_eq!(url.scheme(), "https");
    assert!(matches!(
        url.host_str(),
        Some("auth.openai.com" | "chatgpt.com" | "auth0.openai.com")
    ));
    assert!(login["loginId"].as_str().is_some());
    client
        .call("account/login/cancel", json!({"loginId":login["loginId"]}))
        .await
        .unwrap();
    client.close().await;
}
