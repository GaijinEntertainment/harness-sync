use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit};

#[test]
fn push_to_an_empty_remote_creates_the_first_remote_snapshot() {
    let remote = Remote::new();
    let machine = Machine::new();
    let settings = json!({"effortLevel": "high", "env": {"A": "1"}});
    machine.write_settings(&settings);
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(report["outcome"], "ok");
    assert_eq!(report["items"].as_array().unwrap().len(), 2);
    let head = remote.head("main").expect("the push created branch main");
    assert_eq!(report["remote_commit"], head.as_str());
    assert_eq!(report["base_commit"], head.as_str());
    let snapshot: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(snapshot, settings);
    let (status, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn push_without_differences_creates_no_commit() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    let head = remote.head("main");

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"], json!([]));
    assert_eq!(remote.head("main"), head);
}

#[test]
fn push_from_a_machine_that_has_not_pulled_is_refused() {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let head = remote.head("main");
    let second = Machine::new();
    second.write_settings(&json!({"effortLevel": "low"}));
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = second.harness_sync_json(&["push"]);

    assert_exit(&output, 4);
    assert_eq!(report["outcome"], "remote_ahead");
    assert_eq!(remote.head("main"), head);
}
