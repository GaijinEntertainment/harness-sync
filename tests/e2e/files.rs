use serde_json::json;

use crate::support::{Machine, Remote, assert_exit};

fn connected(remote: &Remote) -> Machine {
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
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
    first.write_file(
        ".claude/skills/.trash/1792400872796-81361-NfxPAe/old/SKILL.md",
        "removed skill\n",
    );
    first.write_file(".claude/rules/.DS_Store", "finder\n");
    first.write_file(".claude/history.jsonl", "{}\n");
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = connected(&remote);

    let output = second.harness_sync(&["pull"]);

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
        "skills/.trash/1792400872796-81361-NfxPAe/old/SKILL.md",
        "rules/.DS_Store",
        "history.jsonl",
    ] {
        assert_eq!(
            second.read_file(&format!(".claude/{skipped}")),
            None,
            "{skipped}"
        );
    }
    let (status, _) = first.harness_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn a_file_deleted_on_one_machine_is_deleted_on_the_other() {
    let remote = Remote::new();
    let first = connected(&remote);
    first.write_file(".claude/rules/old.md", "old rule\n");
    first.write_file(".claude/rules/kept.md", "kept rule\n");
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = connected(&remote);
    assert_exit(&second.harness_sync(&["pull"]), 0);
    std::fs::remove_file(first.home().join(".claude/rules/old.md")).unwrap();
    assert_exit(&first.harness_sync(&["push"]), 0);

    let (report, output) = second.harness_sync_json(&["pull"]);

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

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 5);
    assert_eq!(report["secrets_found"], json!(["file:rules/api.md"]));
    assert_eq!(remote.head("main"), None);
}

#[test]
fn files_that_synced_items_reference_move_too() {
    let remote = Remote::new();
    let first = connected(&remote);
    first.write_settings(
        &json!({"statusLine": {"type": "command", "command": "~/.claude/statusline.sh"}}),
    );
    first.write_file(".claude/statusline.sh", "#!/bin/sh\necho status\n");
    first.set_executable(".claude/statusline.sh");
    first.write_file(
        ".claude/CLAUDE.md",
        "Read [the guide](guides/research.md).\n",
    );
    first.write_file(
        ".claude/guides/research.md",
        "Also @~/.claude/notes/extra.md\n",
    );
    first.write_file(".claude/notes/extra.md", "extra\n");
    first.write_file(".claude/notes/unreferenced.md", "stays\n");
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = connected(&remote);

    let output = second.harness_sync(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(
        second.read_file(".claude/statusline.sh").as_deref(),
        Some("#!/bin/sh\necho status\n")
    );
    assert_eq!(second.is_executable(".claude/statusline.sh"), cfg!(unix));
    assert_eq!(
        second.read_file(".claude/guides/research.md").as_deref(),
        Some("Also @~/.claude/notes/extra.md\n")
    );
    assert_eq!(
        second.read_file(".claude/notes/extra.md").as_deref(),
        Some("extra\n")
    );
    assert_eq!(second.read_file(".claude/notes/unreferenced.md"), None);
    let (status, _) = second.harness_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn exclude_removes_a_referenced_file_and_a_key_from_the_catalog() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "exclude = [\"file:statusline.sh\", \"settings:/model\"]\n",
    );
    let first = connected(&remote);
    first.write_settings(&json!({"model": "opus", "statusLine": {"type": "command", "command": "~/.claude/statusline.sh"}}));
    first.write_file(".claude/statusline.sh", "#!/bin/sh\n");
    assert_exit(&first.harness_sync(&["pull"]), 0);

    let output = first.harness_sync(&["push"]);

    assert_exit(&output, 0);
    let tree =
        String::from_utf8_lossy(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout)
            .into_owned();
    assert!(!tree.contains("statusline.sh"), "tree: {tree}");
    let snapshot: serde_json::Value =
        serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(
        snapshot,
        json!({"statusLine": {"type": "command", "command": "~/.claude/statusline.sh"}})
    );
}

#[test]
fn include_adds_the_files_of_a_directory_but_never_the_credentials() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "include = [\"file:hooks\", \"file:.credentials.json\"]\n",
    );
    let first = connected(&remote);
    first.write_file(".claude/hooks/a.sh", "a\n");
    first.write_file(".claude/hooks/nested/b.sh", "b\n");
    first.write_file(".claude/.credentials.json", "{}\n");
    assert_exit(&first.harness_sync(&["pull"]), 0);

    let output = first.harness_sync(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(remote.show("main:files/hooks/a.sh"), "a\n");
    assert_eq!(remote.show("main:files/hooks/nested/b.sh"), "b\n");
    let tree =
        String::from_utf8_lossy(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout)
            .into_owned();
    assert!(!tree.contains("credentials"), "tree: {tree}");
}
