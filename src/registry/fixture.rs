use serde_json::Value;

use super::Registry;
use crate::harness::{Harness, KeyedFile, Keys, Root};
use crate::item::{ItemId, KeyPath};

pub(super) static REGISTRY: Registry = Registry {
    harnesses: &[&CLAUDE_CODE, &PI],
    storage_kinds: &[],
};

static CLAUDE_CODE: Harness = Harness {
    name: "claude-code",
    roots: &[
        Root {
            path: ".claude.json",
            env: Some("CLAUDE_CONFIG_DIR"),
            in_env_dir: ".claude.json",
        },
        Root {
            path: ".claude",
            env: Some("CLAUDE_CONFIG_DIR"),
            in_env_dir: "",
        },
    ],
    keyed_files: &[
        KeyedFile {
            path: ".claude/settings.json",
            keys: Keys::All,
            split: &["env", "pluginConfigs", "skillOverrides"],
        },
        KeyedFile {
            path: ".claude.json",
            keys: Keys::Only(&["mcpServers", "theme"]),
            split: &["mcpServers"],
        },
    ],
    files: &[],
    dirs: &[],
    skipped_dirs: &[],
    never_referenced: &[".claude/plugins"],
    credentials: &[],
    commands: root_command,
    undeclared_marketplaces: None,
};

static PI: Harness = Harness {
    name: "pi",
    roots: &[Root {
        path: ".pi/agent",
        env: Some("PI_CODING_AGENT_DIR"),
        in_env_dir: "",
    }],
    keyed_files: &[],
    files: &[],
    dirs: &[],
    skipped_dirs: &[],
    never_referenced: &[],
    credentials: &[],
    commands: root_command,
    undeclared_marketplaces: None,
};

fn root_command(id: &ItemId, value: &Value) -> Vec<(KeyPath, String)> {
    let Some(command) = value.get("command").and_then(Value::as_str) else {
        return Vec::new();
    };
    let mut key = id.key_path();
    key.path.push("command".to_owned());
    vec![(key, command.to_owned())]
}
