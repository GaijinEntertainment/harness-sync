use serde_json::json;

use crate::support::{Machine, Remote, assert_exit};

fn connected(remote: &Remote) -> Machine {
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);
    machine
}

#[test]
fn authored_files_move_with_their_mode() {
    let remote = Remote::new();
    let first = connected(&remote);
    first.write_file(".claude/CLAUDE.md", "# Me\n");
    first.write_file(".claude/rules/rust.md", "Use clippy.\n");
    first.write_file(
        ".claude/skills/secret-scanner/SKILL.md",
        "Find leaked keys.\n",
    );
    first.write_file(
        ".claude/skills/secret-scanner/run.sh",
        "#!/bin/sh\necho scan\n",
    );
    first.set_executable(".claude/skills/secret-scanner/run.sh");
    first.write_file(
        ".claude/skills/synced/remote/SKILL.md",
        "claude.ai owns this\n",
    );
    first.write_file(".claude/rules/.DS_Store", "finder\n");
    first.write_file(".claude/history.jsonl", "{}\n");
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = connected(&remote);

    let output = second.cc_sync(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(
        second.read_file(".claude/CLAUDE.md").as_deref(),
        Some("# Me\n")
    );
    assert_eq!(
        second.read_file(".claude/rules/rust.md").as_deref(),
        Some("Use clippy.\n")
    );
    assert_eq!(
        second
            .read_file(".claude/skills/secret-scanner/SKILL.md")
            .as_deref(),
        Some("Find leaked keys.\n")
    );
    assert_eq!(
        second.is_executable(".claude/skills/secret-scanner/run.sh"),
        cfg!(unix)
    );
    for skipped in [
        "skills/synced/remote/SKILL.md",
        "rules/.DS_Store",
        "history.jsonl",
    ] {
        assert_eq!(
            second.read_file(&format!(".claude/{skipped}")),
            None,
            "{skipped}"
        );
    }
    let (status, _) = first.cc_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn a_file_deleted_on_one_machine_is_deleted_on_the_other() {
    let remote = Remote::new();
    let first = connected(&remote);
    first.write_file(".claude/rules/old.md", "old rule\n");
    first.write_file(".claude/rules/kept.md", "kept rule\n");
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = connected(&remote);
    assert_exit(&second.cc_sync(&["pull"]), 0);
    std::fs::remove_file(first.home().join(".claude/rules/old.md")).unwrap();
    assert_exit(&first.cc_sync(&["push"]), 0);

    let (report, output) = second.cc_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"][0]["id"], "file:rules/old.md");
    assert_eq!(second.read_file(".claude/rules/old.md"), None);
    assert_eq!(
        second.read_file(".claude/rules/kept.md").as_deref(),
        Some("kept rule\n")
    );
    assert_eq!(second.backup_files(), ["old rule\n"]);
    assert!(
        remote
            .show("main:files/rules/kept.md")
            .contains("kept rule")
    );
}

#[test]
fn a_token_in_an_authored_file_blocks_the_push() {
    let remote = Remote::new();
    let machine = connected(&remote);
    machine.write_file(
        ".claude/rules/api.md",
        "Call the API with ghp_e2eSecretValue0123456789abcdefABCDEF.\n",
    );

    let (report, output) = machine.cc_sync_json(&["push"]);

    assert_exit(&output, 5);
    assert_eq!(report["secrets_found"], json!(["file:rules/api.md"]));
    assert_eq!(remote.head("main"), None);
}
