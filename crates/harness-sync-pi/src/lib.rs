//! The pi harness of harness-sync (ADR 1.13 holds its catalog).

use harness_sync_core::harness::{Harness, KeyedFile, Keys, Root};
use harness_sync_core::item::{ItemId, KeyPath};
use serde_json::Value;

const MCP: &str = ".pi/agent/mcp.json";

/// The pi harness.
pub static HARNESS: Harness = Harness {
    name: "pi",
    roots: &[Root {
        path: ".pi/agent",
        env: Some("PI_CODING_AGENT_DIR"),
        in_env_dir: "",
    }],
    keyed_files: &[
        KeyedFile {
            path: ".pi/agent/settings.json",
            keys: Keys::Except(&["lastChangelogVersion", "trackingId", "deviceId"]),
            split: &[],
        },
        KeyedFile {
            path: ".pi/agent/models.json",
            keys: Keys::All,
            split: &["providers"],
        },
        KeyedFile {
            path: MCP,
            keys: Keys::All,
            split: &["mcpServers"],
        },
    ],
    files: &[
        ".pi/agent/keybindings.json",
        ".pi/agent/AGENTS.override.md",
        ".pi/agent/AGENTS.md",
        ".pi/agent/CLAUDE.md",
        ".pi/agent/SYSTEM.md",
        ".pi/agent/APPEND_SYSTEM.md",
    ],
    dirs: &[
        ".pi/agent/extensions",
        ".pi/agent/skills",
        ".pi/agent/prompts",
        ".pi/agent/themes",
    ],
    skipped_dirs: &[],
    never_referenced: &[".pi/agent/sessions", ".pi/agent/npm", ".pi/agent/bin"],
    credentials: &[
        ".pi/agent/auth.json",
        ".pi/agent/mcp-auth.json",
        ".pi/agent/models-store.json",
    ],
    commands,
    undeclared_marketplaces: None,
};

fn commands(id: &ItemId, value: &Value) -> Vec<(KeyPath, String)> {
    let is_server = id.file == MCP && id.path.first().is_some_and(|key| key == "mcpServers");
    let Some(command) = value
        .get("command")
        .and_then(Value::as_str)
        .filter(|_| is_server)
    else {
        return Vec::new();
    };
    let mut key = id.key_path();
    key.path.push("command".to_owned());
    vec![(key, command.to_owned())]
}
