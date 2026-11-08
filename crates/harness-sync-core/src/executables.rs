use std::env;
use std::path::{Path, PathBuf};

use crate::harness::{self, Command};
use crate::item::Items;
use crate::report::MissingExecutable;

const SHELL_WORDS: [&str; 13] = [
    "cd", "export", "source", ".", "eval", "if", "for", "while", "set", "unset", "alias", "{", "(",
];

pub(crate) fn missing(setup: &Items, home: &Path) -> Vec<MissingExecutable> {
    commands(setup)
        .into_iter()
        .filter_map(|(id, command)| {
            let executable = match command {
                Command::Shell(line) => executable(&line)?,
                Command::Program(program) => program,
            };
            (!is_installed(&executable, home)).then_some(MissingExecutable { id, executable })
        })
        .collect()
}

fn commands(setup: &Items) -> Vec<(String, Command)> {
    setup
        .iter()
        .filter(|(id, _)| !id.is_whole_file())
        .filter_map(|(id, value)| Some((harness::owner(&id.file)?, id, value)))
        .flat_map(|(harness, id, value)| (harness.commands)(id, value))
        .map(|(key, command)| (key.to_string(), command))
        .collect()
}

fn executable(command: &str) -> Option<String> {
    let word = shell_words(command)
        .into_iter()
        .find(|word| !is_assignment(word) && word != "exec")?;
    (!SHELL_WORDS.contains(&word.as_str())).then_some(word)
}

fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        let escapes = |next: &char| next.is_whitespace() || matches!(next, '"' | '\'');
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (None | Some('"'), '\\') if chars.peek().is_some_and(escapes) => {
                word.get_or_insert_default().extend(chars.next());
            }
            (None, '"' | '\'') => {
                quote = Some(c);
                word.get_or_insert_default();
            }
            (None, c) if c.is_whitespace() => words.extend(word.take()),
            (_, c) => word.get_or_insert_default().push(c),
        }
    }
    words.extend(word);
    words
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
        assert_eq!(executable("exec tool --flag").as_deref(), Some("tool"));
        assert_eq!(executable("exec"), None);
    }

    #[test]
    fn quotes_and_backslashes_keep_the_spaces_of_an_executable_path() {
        assert_eq!(
            executable("\"/Applications/My Tool/bin/x\" --flag").as_deref(),
            Some("/Applications/My Tool/bin/x")
        );
        assert_eq!(
            executable("FOO='a b' '/opt/my tool/x'").as_deref(),
            Some("/opt/my tool/x")
        );
        assert_eq!(
            executable("/opt/my\\ tool/x --flag").as_deref(),
            Some("/opt/my tool/x")
        );
    }

    #[test]
    fn a_backslash_of_a_windows_path_stays_in_the_executable() {
        assert_eq!(
            executable("\"C:\\Program Files\\tool.exe\" --flag").as_deref(),
            Some("C:\\Program Files\\tool.exe")
        );
        assert_eq!(
            executable("C:\\Tools\\tool.exe --flag").as_deref(),
            Some("C:\\Tools\\tool.exe")
        );
    }

    #[test]
    fn a_command_whose_executable_is_absent_is_missing() {
        let setup = settings_items(json!({
            "statusLine": {"type": "command", "command": "sh -c true"},
            "tool": {"command": "harness-sync-test-missing-tool --flag"},
            "script": {"command": "~/.claude/hooks/absent.sh"}
        }))
        .unwrap();

        let missing = missing(&setup, Path::new("/nonexistent-home"));

        assert_eq!(
            missing,
            [
                MissingExecutable {
                    id: ".claude/settings.json:/script/command".to_owned(),
                    executable: "~/.claude/hooks/absent.sh".to_owned()
                },
                MissingExecutable {
                    id: ".claude/settings.json:/tool/command".to_owned(),
                    executable: "harness-sync-test-missing-tool".to_owned()
                },
            ]
        );
    }
}
