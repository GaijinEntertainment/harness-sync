use std::env;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::item::{ItemId, Items, Source};
use crate::report::MissingExecutable;

const SHELL_WORDS: [&str; 14] = [
    "cd", "export", "source", ".", "exec", "eval", "if", "for", "while", "set", "unset", "alias",
    "{", "(",
];

pub(crate) fn missing(setup: &Items, home: &Path) -> Vec<MissingExecutable> {
    commands(setup)
        .into_iter()
        .filter_map(|(id, command)| {
            let executable = executable(&command)?;
            (!is_installed(&executable, home)).then_some(MissingExecutable { id, executable })
        })
        .collect()
}

fn commands(setup: &Items) -> Vec<(String, String)> {
    let mut commands = Vec::new();
    for (id, value) in setup
        .iter()
        .filter(|(id, _)| matches!(id.source, Source::Json(_)))
    {
        match id.path.first().map(String::as_str) {
            Some("statusLine" | "mcpServers") => {
                push_command(&mut commands, id, &["command"], value);
            }
            Some("hooks") => hook_commands(&mut commands, id, value),
            _ => {}
        }
    }
    commands
}

fn hook_commands(commands: &mut Vec<(String, String)>, id: &ItemId, hooks: &Value) {
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

fn push_command(commands: &mut Vec<(String, String)>, id: &ItemId, path: &[&str], value: &Value) {
    let Some(command) = value.get("command").and_then(Value::as_str) else {
        return;
    };
    let mut key = id.key_path();
    key.path
        .extend(path.iter().map(|&segment| segment.to_owned()));
    commands.push((key.to_string(), command.to_owned()));
}

fn executable(command: &str) -> Option<String> {
    let word = command
        .split_whitespace()
        .map(|word| word.trim_matches(['"', '\'']))
        .find(|word| !is_assignment(word))?;
    (!SHELL_WORDS.contains(&word)).then(|| word.to_owned())
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn is_installed(executable: &str, home: &Path) -> bool {
    let expanded = ["~/", "$HOME/", "${HOME}/"]
        .iter()
        .find_map(|prefix| executable.strip_prefix(prefix))
        .map_or_else(|| PathBuf::from(executable), |rest| home.join(rest));
    if executable.contains(['/', '\\']) {
        return expanded.is_file();
    }
    let extensions: Vec<String> = if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_owned())
            .split(';')
            .map(str::to_owned)
            .chain([String::new()])
            .collect()
    } else {
        vec![String::new()]
    };
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|dir| {
            extensions
                .iter()
                .any(|extension| dir.join(format!("{executable}{extension}")).is_file())
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::item::settings_items;

    #[test]
    fn the_executable_is_the_first_word_after_assignments() {
        assert_eq!(executable("node ~/.claude/x.js").as_deref(), Some("node"));
        assert_eq!(
            executable("FOO=1 BAR=x \"bunx\" ccusage").as_deref(),
            Some("bunx")
        );
        assert_eq!(executable("cd ~/x && make"), None);
        assert_eq!(executable("   "), None);
    }

    #[test]
    fn a_status_line_and_hooks_name_their_commands() {
        let setup = settings_items(json!({
            "statusLine": {"type": "command", "command": "sh -c true"},
            "hooks": {"Stop": [{"hooks": [
                {"type": "command", "command": "cc-sync-test-missing-tool --flag"},
                {"type": "command", "command": "~/.claude/hooks/absent.sh"}
            ]}]}
        }))
        .unwrap();

        let missing = missing(&setup, Path::new("/nonexistent-home"));

        assert_eq!(
            missing,
            [
                MissingExecutable {
                    id: "settings:/hooks/Stop/0/hooks/0/command".to_owned(),
                    executable: "cc-sync-test-missing-tool".to_owned()
                },
                MissingExecutable {
                    id: "settings:/hooks/Stop/0/hooks/1/command".to_owned(),
                    executable: "~/.claude/hooks/absent.sh".to_owned()
                },
            ]
        );
    }
}
