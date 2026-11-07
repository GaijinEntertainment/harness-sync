use std::env;
use std::path::{Path, PathBuf};

use crate::harness;
use crate::item::Items;
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
    setup
        .iter()
        .filter(|(id, _)| !id.is_whole_file())
        .filter_map(|(id, value)| Some((harness::owner(&id.file)?, id, value)))
        .flat_map(|(harness, id, value)| (harness.commands)(id, value))
        .map(|(key, command)| (key.to_string(), command))
        .collect()
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
                {"type": "command", "command": "harness-sync-test-missing-tool --flag"},
                {"type": "command", "command": "~/.claude/hooks/absent.sh"}
            ]}]}
        }))
        .unwrap();

        let missing = missing(&setup, Path::new("/nonexistent-home"));

        assert_eq!(
            missing,
            [
                MissingExecutable {
                    id: ".claude/settings.json:/hooks/Stop/0/hooks/0/command".to_owned(),
                    executable: "harness-sync-test-missing-tool".to_owned()
                },
                MissingExecutable {
                    id: ".claude/settings.json:/hooks/Stop/0/hooks/1/command".to_owned(),
                    executable: "~/.claude/hooks/absent.sh".to_owned()
                },
            ]
        );
    }
}
