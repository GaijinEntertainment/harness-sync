use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit};

const PROVIDER_KEY: &str = "acme-provider-value";
const AUTH_VALUE: &str = "acme-auth-value";

fn pi_json(machine: &Machine, file: &str) -> Value {
    let text = machine
        .read_file(&format!(".pi/agent/{file}"))
        .unwrap_or_else(|| panic!("{file} exists"));
    serde_json::from_str(&text).unwrap()
}

fn snapshot_json(remote: &Remote, file: &str) -> Value {
    serde_json::from_str(&remote.show(&format!("main:.pi/agent/{file}"))).unwrap()
}

fn pushed_pi_setup(remote: &Remote) {
    let first = Machine::new();
    first.write_file(
        ".pi/agent/settings.json",
        r#"{"defaultModel": "opus", "lastChangelogVersion": "1.0", "packages": ["npm:pi-tools"]}"#,
    );
    first.write_file(
        ".pi/agent/models.json",
        &format!(
            r#"{{"providers": {{"acme": {{"baseUrl": "https://api.acme.test", "apiKey": "{PROVIDER_KEY}"}}}}}}"#
        ),
    );
    first.write_file(
        ".pi/agent/mcp.json",
        r#"{"mcpServers": {"kartta": {"command": "sh", "args": ["-c", "true"]}}}"#,
    );
    first.write_file(".pi/agent/AGENTS.md", "Be brief.\n");
    first.write_file(".pi/agent/extensions/notify.ts", "export default {};\n");
    first.write_file(
        ".pi/agent/extensions/node_modules/dep/index.js",
        "module.exports = 1;\n",
    );
    first.write_file(
        ".pi/agent/auth.json",
        &format!(r#"{{"acme": "{AUTH_VALUE}"}}"#),
    );
    first.write_file(".pi/agent/sessions/one.jsonl", "{}\n");
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
}

#[test]
fn pi_settings_providers_servers_and_files_move_and_its_state_stays() {
    let remote = Remote::new();
    pushed_pi_setup(&remote);
    let second = Machine::without_harnesses();
    second.write_file(
        ".pi/agent/settings.json",
        r#"{"lastChangelogVersion": "2.0"}"#,
    );
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);

    let (pulled, output) = second.harness_sync_json(&["pull"]);

    assert_exit(&output, 0);
    let objects = remote.all_objects();
    for private in [
        PROVIDER_KEY,
        AUTH_VALUE,
        "lastChangelogVersion",
        "module.exports = 1;",
    ] {
        assert!(!objects.contains(private), "the repo holds {private}");
    }
    assert_eq!(
        snapshot_json(&remote, "models.json")["providers"]["acme"]["apiKey"],
        "<harness-sync secret .pi/agent/models.json:/providers/acme/apiKey>"
    );
    let settings = pi_json(&second, "settings.json");
    assert_eq!(settings["lastChangelogVersion"], "2.0");
    assert_eq!(settings["defaultModel"], "opus");
    assert_eq!(settings["packages"], json!(["npm:pi-tools"]));
    assert_eq!(
        pi_json(&second, "models.json"),
        json!({"providers": {"acme": {"baseUrl": "https://api.acme.test"}}})
    );
    assert_eq!(
        pi_json(&second, "mcp.json")["mcpServers"]["kartta"]["command"],
        "sh"
    );
    assert_eq!(
        second.read_file(".pi/agent/AGENTS.md").as_deref(),
        Some("Be brief.\n")
    );
    assert_eq!(
        second
            .read_file(".pi/agent/extensions/notify.ts")
            .as_deref(),
        Some("export default {};\n")
    );
    assert!(
        pulled["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |item| item["id"] == ".pi/agent/models.json:/providers/acme/apiKey"
                    && item["state"] == "secret_missing"
            )
    );
    assert!(!second.home().join(".claude").exists());
}

#[test]
fn a_machine_with_pi_alone_exchanges_pi_items_and_keeps_the_claude_code_items() {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_settings(&json!({"model": "opus"}));
    first.write_file(
        ".pi/agent/models.json",
        r#"{"providers": {"acme": {"baseUrl": "https://a.test"}}}"#,
    );
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = Machine::without_harnesses();
    second.write_file(".pi/agent/models.json", r#"{"providers": {}}"#);
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&second.harness_sync(&["pull"]), 0);
    second.write_file(
        ".pi/agent/models.json",
        r#"{"providers": {"acme": {"baseUrl": "https://b.test"}}}"#,
    );
    assert_exit(&second.harness_sync(&["push"]), 0);

    let output = first.harness_sync(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(
        pi_json(&first, "models.json")["providers"]["acme"]["baseUrl"],
        "https://b.test"
    );
    assert_eq!(first.settings(), json!({"model": "opus"}));
    assert!(remote.show("main:.claude/settings.json").contains("opus"));
    assert!(!second.home().join(".claude").exists());
}
