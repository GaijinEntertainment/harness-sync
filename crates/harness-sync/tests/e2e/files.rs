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
    first.write_file(
        ".claude/skills/secret-scanner/node_modules/dep/index.js",
        "module.exports = 1;\n",
    );
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
        "skills/secret-scanner/node_modules/dep/index.js",
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
    assert_eq!(report["items"][0]["id"], ".claude/rules/old.md");
    assert_eq!(second.read_file(".claude/rules/old.md"), None);
    assert_eq!(
        second.read_file(".claude/rules/kept.md").as_deref(),
        Some("kept rule\n")
    );
    assert_eq!(second.backup_files(), ["old rule\n"]);
    assert!(
        remote
            .show("main:.claude/rules/kept.md")
            .contains("kept rule")
    );
}

#[test]
fn a_repo_file_outside_the_harness_directories_never_reaches_the_home_directory() {
    let remote = Remote::new();
    remote.commit_file("main", "README.md", "About this repo.\n");
    let machine = connected(&remote);

    let (report, output) = machine.harness_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"], json!([]));
    assert_eq!(machine.read_file("README.md"), None);
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
    assert_eq!(report["secrets_found"], json!([".claude/rules/api.md"]));
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
        "exclude = [\".claude/statusline.sh\", \".claude/settings.json:/model\"]\n",
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
        serde_json::from_str(&remote.show("main:.claude/settings.json")).unwrap();
    assert_eq!(
        snapshot,
        json!({"statusLine": {"type": "command", "command": "~/.claude/statusline.sh"}})
    );
}

#[test]
fn include_adds_the_files_of_a_directory_but_never_a_file_that_never_syncs() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "include = [\".claude\", \".claude/.credentials.json\", \".harness-sync\", \
         \"tools/node_modules/dep/index.js\", \"notes/.DS_Store\"]\n",
    );
    let first = connected(&remote);
    first.write_settings(&json!({"model": "opus"}));
    first.write_file(".claude/hooks/a.sh", "a\n");
    first.write_file(".claude/hooks/nested/b.sh", "b\n");
    first.write_file(".claude/.credentials.json", "{}\n");
    first.write_file("tools/node_modules/dep/index.js", "module.exports = 1;\n");
    first.write_file("notes/.DS_Store", "finder\n");
    assert_exit(&first.harness_sync(&["pull"]), 0);

    let output = first.harness_sync(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(remote.show("main:.claude/hooks/a.sh"), "a\n");
    assert_eq!(remote.show("main:.claude/hooks/nested/b.sh"), "b\n");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&remote.show("main:.claude/settings.json"))
            .unwrap(),
        json!({"model": "opus"})
    );
    let tree =
        String::from_utf8_lossy(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout)
            .into_owned();
    for never in ["credentials", ".harness-sync/", "node_modules", ".DS_Store"] {
        assert!(!tree.contains(never), "{never} in tree: {tree}");
    }
}

#[cfg(unix)]
fn link(machine: &Machine, path: &str, target: &str) {
    std::os::unix::fs::symlink(machine.home().join(target), machine.home().join(path)).unwrap();
}

#[cfg(unix)]
#[test]
fn push_skips_a_file_behind_a_symlink() {
    let remote = Remote::new();
    let machine = connected(&remote);
    machine.write_file("dotfiles/skills/x/SKILL.md", "linked skill\n");
    machine.write_file("dotfiles/settings.json", "{\"model\": \"opus\"}\n");
    machine.write_file("dotfiles/linked.md", "linked rule\n");
    machine.write_file("dotfiles/hooks/x.sh", "#!/bin/sh\n");
    machine.write_file(".claude/rules/real.md", "Run [the hook](../hooks/x.sh).\n");
    link(&machine, ".claude/skills", "dotfiles/skills");
    link(&machine, ".claude/settings.json", "dotfiles/settings.json");
    link(&machine, ".claude/rules/linked.md", "dotfiles/linked.md");
    link(&machine, ".claude/hooks", "dotfiles/hooks");

    let output = machine.harness_sync(&["push"]);

    assert_exit(&output, 0);
    let tree =
        String::from_utf8_lossy(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout)
            .into_owned();
    assert_eq!(tree, ".claude/rules/real.md\n");
}

#[cfg(unix)]
#[test]
fn pull_never_writes_through_a_symlink() {
    let remote = Remote::new();
    let first = connected(&remote);
    first.write_settings(&json!({"model": "opus"}));
    first.write_file(".claude/skills/x/SKILL.md", "skill\n");
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = connected(&remote);
    second.write_file("dotfiles/settings.json", "{\"model\": \"sonnet\"}\n");
    std::fs::create_dir_all(second.home().join("dotfiles/skills")).unwrap();
    link(&second, ".claude/skills", "dotfiles/skills");
    link(&second, ".claude/settings.json", "dotfiles/settings.json");

    let (report, output) = second.harness_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"], json!([]));
    assert_eq!(second.read_file("dotfiles/skills/x/SKILL.md"), None);
    assert_eq!(
        second.read_file("dotfiles/settings.json").as_deref(),
        Some("{\"model\": \"sonnet\"}\n")
    );
    let secret = second.harness_sync_with_input(
        &["secret", "set", ".claude/settings.json:/env/GITHUB_TOKEN"],
        "ghp_e2eSecretValue0123456789abcdefABCDEF\n",
    );
    assert_exit(&secret, 1);
    assert_eq!(
        second.read_file("dotfiles/settings.json").as_deref(),
        Some("{\"model\": \"sonnet\"}\n")
    );
    assert_exit(&second.harness_sync(&["push"]), 0);
    assert_eq!(remote.show("main:.claude/skills/x/SKILL.md"), "skill\n");
}

#[test]
fn pull_ignores_a_remote_file_that_never_syncs() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "include = [\".harness-sync\", \".claude/.credentials.json\"]\n",
    );
    let work_tree_can_hold = |file: &&str| cfg!(unix) || !file.contains([':', '\\']);
    for file in [
        ".harness-sync/config.toml",
        ".claude/.credentials.json",
        ".claude/rules/a:b.md",
        ".claude/rules/a\\b.md",
        ".claude/rules/node_modules/dep/index.js",
    ]
    .into_iter()
    .filter(work_tree_can_hold)
    {
        remote.commit_file("main", file, "from the remote\n");
    }
    let machine = connected(&remote);
    let config = machine.read_file(".harness-sync/config.toml");

    let (report, output) = machine.harness_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"], json!([]));
    assert_eq!(machine.read_file(".harness-sync/config.toml"), config);
    for file in [
        ".claude/.credentials.json",
        ".claude/rules/a:b.md",
        ".claude/rules/a\\b.md",
        ".claude/rules/node_modules/dep/index.js",
    ] {
        assert_eq!(machine.read_file(file), None, "{file}");
    }
}
