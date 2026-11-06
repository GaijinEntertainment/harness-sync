use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit};

const FIRST: &str = r#"{
  "numStartups": 41,
  "oauthAccount": {"emailAddress": "first@example.com", "accountUuid": "uuid-first"},
  "projects": {"/work/app": {"allowedTools": [], "lastSessionId": "s-1"}},
  "theme": "dark",
  "mcpServers": {"kartta": {"type": "stdio", "command": "sh", "args": ["-c", "true"]}}
}
"#;

const SECOND: &str = r#"{
	"numStartups": 3,
	"userID": "second-user",
	"theme": "light"
}
"#;

#[test]
fn catalog_keys_of_claude_json_move_and_the_rest_stays_on_each_machine() {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_file(".claude.json", FIRST);
    assert_exit(&first.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = Machine::new();
    second.write_file(".claude.json", SECOND);
    assert_exit(&second.cc_sync(&["init", &remote.url()]), 0);

    let output = second.cc_sync(&["pull", "--prefer", "remote"]);

    assert_exit(&output, 0);
    let objects = remote.all_objects();
    assert!(objects.contains("kartta"), "objects: {objects}");
    for private in [
        "first@example.com",
        "uuid-first",
        "numStartups",
        "/work/app",
        "s-1",
    ] {
        assert!(!objects.contains(private), "the repo holds {private}");
    }
    let snapshot: Value = serde_json::from_str(&remote.show("main:claude.json")).unwrap();
    assert_eq!(
        snapshot,
        json!({"mcpServers": {"kartta": {"type": "stdio", "command": "sh", "args": ["-c", "true"]}}, "theme": "dark"})
    );
    let text = second.read_file(".claude.json").unwrap();
    assert!(
        text.starts_with(
            "{\n\t\"numStartups\": 3,\n\t\"userID\": \"second-user\",\n\t\"theme\": \"dark\""
        ),
        "{text}"
    );
    let pulled: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(pulled["mcpServers"]["kartta"]["command"], "sh");
}

#[test]
fn claude_config_dir_moves_both_claude_code_files() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_file("alt/settings.json", r#"{"model": "opus"}"#);
    machine.write_file("alt/.claude.json", r#"{"theme": "dark"}"#);
    machine.write_file(".claude/settings.json", r#"{"model": "ignored"}"#);
    machine.write_file(".claude.json", r#"{"theme": "ignored"}"#);
    assert_exit(
        &machine.cc_sync_in_config_dir("alt", &["init", &remote.url()]),
        0,
    );

    let output = machine.cc_sync_in_config_dir("alt", &["push"]);

    assert_exit(&output, 0);
    let settings: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    let claude: Value = serde_json::from_str(&remote.show("main:claude.json")).unwrap();
    assert_eq!(settings, json!({"model": "opus"}));
    assert_eq!(claude, json!({"theme": "dark"}));
}
