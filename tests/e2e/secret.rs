use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit};

const TOKEN: &str = "ghp_e2eSecretValue0123456789abcdefABCDEF";
const PLACEHOLDER: &str = "<cc-sync secret settings:/env/GITHUB_TOKEN>";

fn connected(remote: &Remote, settings: &Value) -> Machine {
    let machine = Machine::new();
    machine.write_settings(settings);
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);
    machine
}

#[test]
fn a_secret_never_reaches_the_repo_and_each_machine_keeps_its_own_value() {
    let remote = Remote::new();
    let first = connected(
        &remote,
        &json!({"env": {"GITHUB_TOKEN": TOKEN, "EDITOR": "vim"}}),
    );
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = connected(&remote, &json!({"env": {"GITHUB_TOKEN": "own"}}));

    let output = second.cc_sync(&["pull"]);

    assert_exit(&output, 0);
    let objects = remote.all_objects();
    assert!(objects.contains("GITHUB_TOKEN"), "objects: {objects}");
    assert!(!objects.contains(TOKEN));
    let snapshot: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(snapshot["env"]["GITHUB_TOKEN"], PLACEHOLDER);
    assert_eq!(
        second.settings(),
        json!({"env": {"GITHUB_TOKEN": "own", "EDITOR": "vim"}})
    );
    let (status, _) = second.cc_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn a_missing_secret_is_listed_and_is_not_a_local_deletion() {
    let remote = Remote::new();
    let first = connected(&remote, &json!({"env": {"GITHUB_TOKEN": TOKEN}}));
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = connected(&remote, &json!({}));

    let (pulled, output) = second.cc_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(second.settings(), json!({}));
    let missing = json!({
        "id": "settings:/env/GITHUB_TOKEN",
        "state": "secret_missing",
        "local": null,
        "remote": PLACEHOLDER
    });
    assert_eq!(pulled["items"].as_array().unwrap().last(), Some(&missing));
    let (status, _) = second.cc_sync_json(&["status"]);
    assert_eq!(status["items"], json!([missing]));
    second.write_settings(&json!({"model": "opus"}));
    assert_exit(&second.cc_sync(&["push"]), 0);
    let snapshot: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(
        snapshot,
        json!({"env": {"GITHUB_TOKEN": PLACEHOLDER}, "model": "opus"})
    );
}

#[test]
fn a_token_outside_a_placeholder_blocks_the_push() {
    let remote = Remote::new();
    let machine = connected(&remote, &json!({"env": {"NOTE": TOKEN}, "model": "opus"}));

    let (report, output) = machine.cc_sync_json(&["push"]);

    assert_exit(&output, 5);
    assert_eq!(report["outcome"], "secrets_found");
    assert_eq!(report["secrets_found"], json!(["settings:/env/NOTE"]));
    assert!(!String::from_utf8_lossy(&output.stdout).contains(TOKEN));
    assert_eq!(remote.head("main"), None);
}
