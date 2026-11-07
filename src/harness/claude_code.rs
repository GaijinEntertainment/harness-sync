use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use super::{Harness, KeyedFile, Keys, Root};
use crate::declarations::Declarations;
use crate::item::{ItemId, Items, KeyPath};
use crate::paths::Paths;
use crate::report::UndeclaredMarketplace;
use crate::secret;

const CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";
const SETTINGS: &str = ".claude/settings.json";
const KNOWN_MARKETPLACES: &str = ".claude/plugins/known_marketplaces.json";
const MARKETPLACE_DECLARATIONS: &str = "extraKnownMarketplaces";
const MARKETPLACE_DECLARATION_FIELDS: [&str; 2] = ["source", "autoUpdate"];

const SPLIT_SETTINGS_KEYS: [&str; 6] = [
    "env",
    "enabledPlugins",
    MARKETPLACE_DECLARATIONS,
    "pluginConfigs",
    "skillOverrides",
    "modelSettings",
];

const CLAUDE_JSON_KEYS: [&str; 29] = [
    "mcpServers",
    "autoConnectIde",
    "autoInstallIdeExtension",
    "claudeInChromeDefaultEnabled",
    "copyFullResponse",
    "copyOnSelect",
    "defaultToAgentsView",
    "diffTool",
    "externalEditorContext",
    "leftArrowOpensAgents",
    "permissionExplainerEnabled",
    "prStatusFooterEnabled",
    "teammateDefaultModel",
    "respectGitignore",
    "showTurnDuration",
    "terminalProgressBarEnabled",
    "theme",
    "verbose",
    "workflowSizeGuideline",
    "teammateMode",
    "agentPushNotifEnabled",
    "inputNeededNotifEnabled",
    "preferredNotifChannel",
    "remoteControlAtStartup",
    "showSpinnerTree",
    "diffSidebarOpen",
    "deepLinkTerminal",
    "lspRecommendationDisabled",
    "companionMuted",
];

pub(crate) static HARNESS: Harness = Harness {
    name: "claude-code",
    roots: &[
        Root {
            path: ".claude.json",
            env: Some(CONFIG_DIR_ENV),
            in_env_dir: ".claude.json",
        },
        Root {
            path: ".claude",
            env: Some(CONFIG_DIR_ENV),
            in_env_dir: "",
        },
    ],
    keyed_files: &[
        KeyedFile {
            path: SETTINGS,
            keys: Keys::All,
            split: &SPLIT_SETTINGS_KEYS,
        },
        KeyedFile {
            path: ".claude.json",
            keys: Keys::Only(&CLAUDE_JSON_KEYS),
            split: &["mcpServers"],
        },
    ],
    files: &[".claude/CLAUDE.md", ".claude/keybindings.json"],
    dirs: &[
        ".claude/rules",
        ".claude/skills",
        ".claude/agents",
        ".claude/commands",
        ".claude/output-styles",
        ".claude/workflows",
        ".claude/themes",
    ],
    skipped_dirs: &[".claude/skills/synced", ".claude/skills/.trash"],
    never_referenced: &[
        ".claude/plugins",
        ".claude/projects",
        ".claude/skills/synced",
    ],
    credentials: &[".claude/.credentials.json"],
    commands,
    undeclared_marketplaces: Some(undeclared_marketplaces),
};

fn commands(id: &ItemId, value: &Value) -> Vec<(KeyPath, String)> {
    let mut commands = Vec::new();
    match id.path.first().map(String::as_str) {
        Some("statusLine" | "mcpServers") => push_command(&mut commands, id, &["command"], value),
        Some("hooks") => hook_commands(&mut commands, id, value),
        _ => {}
    }
    commands
}

fn hook_commands(commands: &mut Vec<(KeyPath, String)>, id: &ItemId, hooks: &Value) {
    let matchers = hooks
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(event, matchers)| {
            let list = matchers.as_array().into_iter().flatten().enumerate();
            list.map(move |(index, matcher)| (event, index, matcher))
        });
    for (event, index, matcher) in matchers {
        let hooks = matcher.get("hooks").and_then(Value::as_array);
        for (position, hook) in hooks.into_iter().flatten().enumerate() {
            let path = [
                event.as_str(),
                &index.to_string(),
                "hooks",
                &position.to_string(),
                "command",
            ];
            push_command(commands, id, &path, hook);
        }
    }
}

fn push_command(commands: &mut Vec<(KeyPath, String)>, id: &ItemId, path: &[&str], value: &Value) {
    let Some(command) = value.get("command").and_then(Value::as_str) else {
        return;
    };
    let mut key = id.key_path();
    key.path
        .extend(path.iter().map(|&segment| segment.to_owned()));
    commands.push((key, command.to_owned()));
}

fn undeclared_marketplaces(
    paths: &Paths,
    setup: &Items,
    declarations: &Declarations,
) -> Result<Vec<UndeclaredMarketplace>> {
    let path = paths.local(KNOWN_MARKETPLACES);
    let known: Map<String, Value> = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("cannot read the marketplaces of {}", path.display()))?,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    Ok(known
        .into_iter()
        .filter_map(|(name, record)| {
            let id = ItemId::key(SETTINGS, &[MARKETPLACE_DECLARATIONS, &name]);
            if setup.contains_key(&id) || declarations.excludes(&id) {
                return None;
            }
            let declaration: Map<String, Value> = record
                .as_object()?
                .iter()
                .filter(|(field, _)| MARKETPLACE_DECLARATION_FIELDS.contains(&field.as_str()))
                .map(|(field, value)| (field.clone(), value.clone()))
                .collect();
            Some(UndeclaredMarketplace {
                declaration: secret::redact(&id, &Value::Object(declaration)),
                name,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn named(id: &ItemId, value: &Value) -> Vec<(String, String)> {
        commands(id, value)
            .into_iter()
            .map(|(key, command)| (key.to_string(), command))
            .collect()
    }

    #[test]
    fn a_status_line_an_mcp_server_and_hooks_name_their_commands() {
        let status_line = ItemId::key(SETTINGS, &["statusLine"]);
        let server = ItemId::key(".claude.json", &["mcpServers", "kartta"]);
        let hooks = ItemId::key(SETTINGS, &["hooks"]);
        let hooks_value = json!({"Stop": [{"hooks": [
            {"type": "command", "command": "tool --flag"},
            {"type": "command", "command": "~/.claude/hooks/x.sh"}
        ]}]});

        assert_eq!(
            named(
                &status_line,
                &json!({"type": "command", "command": "sh -c true"})
            ),
            [(
                ".claude/settings.json:/statusLine/command".to_owned(),
                "sh -c true".to_owned()
            )]
        );
        assert_eq!(
            named(&server, &json!({"command": "kartta"})),
            [(
                ".claude.json:/mcpServers/kartta/command".to_owned(),
                "kartta".to_owned()
            )]
        );
        assert_eq!(
            named(&hooks, &hooks_value),
            [
                (
                    ".claude/settings.json:/hooks/Stop/0/hooks/0/command".to_owned(),
                    "tool --flag".to_owned()
                ),
                (
                    ".claude/settings.json:/hooks/Stop/0/hooks/1/command".to_owned(),
                    "~/.claude/hooks/x.sh".to_owned()
                ),
            ]
        );
        assert_eq!(
            named(&ItemId::key(SETTINGS, &["model"]), &json!("opus")),
            []
        );
    }
}
