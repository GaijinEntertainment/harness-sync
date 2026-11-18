use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit, text};

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
    let snapshot: Value = serde_json::from_str(&remote.show("main:.claude/settings.json")).unwrap();
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
    assert_eq!(
        report["items"],
        json!([{"id": ".claude/settings.json:/effortLevel", "state": "conflict", "local": "low", "remote": "high"}])
    );
    assert_eq!(remote.head("main"), head);
}

#[cfg(unix)]
#[test]
fn push_that_meets_a_commit_made_during_the_push_reports_its_items() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert!(remote.git(&["branch", "ahead", "main"]).status.success());
    remote.commit_file("ahead", ".claude/rules/other.md", "Another machine.\n");
    machine.write_file(
        "hooks/pre-push",
        &format!(
            "#!/bin/sh\ngit --git-dir='{}' update-ref refs/heads/main refs/heads/ahead\n",
            remote.url()
        ),
    );
    machine.set_executable("hooks/pre-push");
    let config = machine.read_file(".gitconfig").unwrap();
    let hooks = machine.home().join("hooks");
    machine.write_file(
        ".gitconfig",
        &format!("{config}[core]\n\thooksPath = {}\n", hooks.display()),
    );
    machine.write_settings(&json!({"effortLevel": "low"}));

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 4);
    assert_eq!(report["outcome"], "remote_ahead");
    assert_eq!(
        report["items"],
        json!([{"id": ".claude/settings.json:/effortLevel", "state": "local_change", "local": "low", "remote": "high"}])
    );
    assert_eq!(remote.head("main"), remote.head("ahead"));
}

#[test]
fn push_with_only_pinned_items_that_differ_says_so() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert_exit(
        &machine.harness_sync(&["pin", ".claude/settings.json:/model"]),
        0,
    );
    machine.write_settings(&json!({"model": "sonnet"}));

    let output = machine.harness_sync(&["push"]);

    assert_exit(&output, 0);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("nothing to push: only pinned items differ from the remote snapshot")
    );
}

#[test]
fn push_lists_the_unmanaged_files_of_the_remote_snapshot_and_keeps_them() {
    let remote = Remote::new();
    remote.commit_file("main", "README.md", "About this repo.\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["pull"]), 0);
    machine.write_settings(&json!({"effortLevel": "high"}));

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(report["unmanaged_files"], json!(["README.md"]));
    assert_eq!(remote.show("main:README.md"), "About this repo.\n");
}

#[test]
fn push_with_prune_removes_only_the_unmanaged_files() {
    let remote = Remote::new();
    remote.commit_file("main", "harness-sync.toml", "include = [\".agents\"]\n");
    remote.commit_file("main", ".agents/skills/a.md", "a skill\n");
    remote.commit_file("main", ".pi/agent/models.json", "{}\n");
    remote.commit_file("main", "README.md", "About this repo.\n");
    remote.commit_file("main", "files/CLAUDE.md", "old layout\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["pull"]), 0);
    machine.write_settings(&json!({"effortLevel": "high"}));

    let (report, output) = machine.harness_sync_json(&["push", "--prune"]);

    assert_exit(&output, 0);
    assert_eq!(report.get("unmanaged_files"), None);
    let kept = text(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout);
    assert_eq!(
        kept.lines().collect::<Vec<_>>(),
        [
            ".agents/skills/a.md",
            ".claude/settings.json",
            ".pi/agent/models.json",
            "harness-sync.toml"
        ]
    );
    let (status, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
    assert_eq!(status.get("unmanaged_files"), None);
}

#[test]
fn push_with_prune_and_no_changed_item_still_removes_the_unmanaged_files() {
    let remote = Remote::new();
    remote.commit_file("main", "README.md", "About this repo.\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["pull"]), 0);

    let pruned = machine.harness_sync(&["push", "--prune"]);
    let head = remote.head("main");
    let again = machine.harness_sync(&["push", "--prune"]);

    assert_exit(&pruned, 0);
    assert!(
        text(&pruned.stdout).contains("pushed 0 items and the removal of 1 unmanaged files"),
        "stdout: {}",
        text(&pruned.stdout)
    );
    assert!(
        !remote
            .git(&["cat-file", "-e", "main:README.md"])
            .status
            .success()
    );
    assert_exit(&again, 0);
    assert_eq!(remote.head("main"), head);
}

#[test]
fn push_with_prune_is_refused_when_the_remote_is_ahead() {
    let remote = Remote::new();
    remote.commit_file("main", "README.md", "About this repo.\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    let head = remote.head("main");

    let (report, output) = machine.harness_sync_json(&["push", "--prune"]);

    assert_exit(&output, 4);
    assert_eq!(report["unmanaged_files"], json!(["README.md"]));
    assert_eq!(remote.head("main"), head);
}

#[test]
fn push_that_keeps_unmanaged_files_names_the_prune_flag() {
    let remote = Remote::new();
    remote.commit_file("main", "README.md", "About this repo.\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["pull"]), 0);

    let kept = machine.harness_sync(&["push"]);
    let pruned = machine.harness_sync(&["push", "--prune"]);

    assert!(
        text(&kept.stderr).contains("`harness-sync push --prune` removes the unmanaged files"),
        "stderr: {}",
        text(&kept.stderr)
    );
    assert_eq!(text(&pruned.stderr), "");
}

#[cfg(unix)]
#[test]
fn push_with_prune_removes_a_file_whose_path_git_must_quote() {
    let remote = Remote::new();
    remote.commit_file("main", "line\nbreak.md", "x\n");
    remote.commit_file("main", "\"quoted\".md", "x\n");
    remote.commit_file("main", "back\\slash.md", "x\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["pull"]), 0);

    let output = machine.harness_sync(&["push", "--prune"]);

    assert_exit(&output, 0);
    let kept = text(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout);
    assert_eq!(kept, "");
}

#[test]
fn push_to_another_target_lists_the_unmanaged_files_of_that_target() {
    let primary = Remote::new();
    let mirror = Remote::new();
    mirror.commit_file("main", "README.md", "About the mirror.\n");
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &primary.url()]), 0);
    assert_exit(
        &machine.harness_sync(&["target", "add", "mirror", &mirror.url()]),
        0,
    );

    let (report, output) = machine.harness_sync_json(&["push", "--to", "all"]);

    assert_exit(&output, 0);
    assert_eq!(report.get("unmanaged_files"), None);
    assert_eq!(report["pushes"][0].get("unmanaged_files"), None);
    assert_eq!(report["pushes"][1]["unmanaged_files"], json!(["README.md"]));
    let again = machine.harness_sync(&["push", "--to", "mirror"]);
    assert!(
        String::from_utf8_lossy(&again.stderr).contains(
            "warning: the snapshot of the target `mirror` holds README.md, which harness-sync does not manage"
        ),
        "stderr: {}",
        String::from_utf8_lossy(&again.stderr)
    );
}
