use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::harness::{self, keyed_file};
use crate::item::{Items, is_within};
use crate::paths::Paths;

const WORD_SEPARATORS: [char; 11] = [' ', '\t', '\n', '"', '\'', '=', '(', ')', ';', '&', '|'];

pub(crate) fn find(paths: &Paths, json: &Items, catalog: &[String]) -> Vec<String> {
    let mut found = BTreeSet::new();
    let mut markdown: Vec<String> = catalog
        .iter()
        .filter(|path| is_markdown(path))
        .cloned()
        .collect();
    let words = json
        .values()
        .flat_map(strings)
        .flat_map(|text| text.split(WORD_SEPARATORS));
    for relative in words.filter_map(|word| resolve(paths, word, None)) {
        remember(relative, &mut found, &mut markdown);
    }
    let mut scanned = BTreeSet::new();
    while let Some(file) = markdown.pop() {
        if !scanned.insert(file.clone()) {
            continue;
        }
        let path = paths.local(&file);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let dir = path.parent().map(Path::to_path_buf);
        for target in markdown_targets(&text) {
            if let Some(relative) = resolve(paths, target, dir.as_deref()) {
                remember(relative, &mut found, &mut markdown);
            }
        }
    }
    found
        .into_iter()
        .filter(|path| !catalog.contains(path))
        .collect()
}

fn remember(relative: String, found: &mut BTreeSet<String>, markdown: &mut Vec<String>) {
    if is_markdown(&relative) {
        markdown.push(relative.clone());
    }
    found.insert(relative);
}

fn is_markdown(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn markdown_targets(text: &str) -> Vec<&str> {
    let imports = text
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('@'))
        .map(|path| path.trim_end_matches([',', '.', ';', ':', ')']));
    let links = text.split("](").skip(1).filter_map(|rest| {
        let target = rest.split(')').next()?.split_whitespace().next()?;
        let target = target.trim_start_matches('<').trim_end_matches('>');
        let target = target.split('#').next()?;
        (!target.contains("://") && !target.starts_with("mailto:")).then_some(target)
    });
    imports
        .chain(links)
        .filter(|target| !target.is_empty())
        .collect()
}

fn resolve(paths: &Paths, raw: &str, base: Option<&Path>) -> Option<String> {
    let expanded = ["~/", "$HOME/", "${HOME}/"]
        .iter()
        .find_map(|prefix| raw.strip_prefix(prefix))
        .map(|rest| paths.home.join(rest));
    let path = match expanded {
        Some(path) => path,
        None if Path::new(raw).is_absolute() => PathBuf::from(raw),
        None => base?.join(raw),
    };
    let normal = normalize(&path)?;
    let file = paths.home_path(&normal)?;
    let harness = harness::owner(&file)?;
    let allowed = paths.local(&file) == normal
        && keyed_file(&file).is_none()
        && !harness::is_credentials(&file)
        && !harness
            .never_referenced
            .iter()
            .any(|dir| is_within(&file, dir));
    (allowed && fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()))
        .then_some(file)
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !normal.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
    }
    Some(normal)
}

fn strings(value: &Value) -> Vec<&str> {
    match value {
        Value::String(text) => vec![text.as_str()],
        Value::Object(entries) => entries.values().flat_map(strings).collect(),
        Value::Array(elements) => elements.iter().flat_map(strings).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_targets_are_imports_and_relative_links() {
        let text = "See @rules/rust.md, and [guide](guides/research.md#top \"Guide\").\n\
                    Mail me@example.com or read [docs](https://example.com) and [self](#top).";

        assert_eq!(
            markdown_targets(text),
            ["rules/rust.md", "guides/research.md"]
        );
    }

    #[test]
    fn a_path_resolves_inside_the_claude_directory_only() {
        let home = tempfile::tempdir().unwrap();
        let claude_dir = home.path().join(".claude");
        for file in ["hooks/x.sh", "plugins/cache/p/run.sh", "rules/a.md"] {
            let path = claude_dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        fs::write(home.path().join("outside.sh"), "x").unwrap();
        let paths = Paths::for_home(home.path());

        assert_eq!(
            resolve(&paths, "~/.claude/hooks/x.sh", None).as_deref(),
            Some(".claude/hooks/x.sh")
        );
        assert_eq!(
            resolve(&paths, "../hooks/x.sh", Some(&claude_dir.join("rules"))).as_deref(),
            Some(".claude/hooks/x.sh")
        );
        assert_eq!(
            resolve(&paths, "~/.claude/plugins/cache/p/run.sh", None),
            None
        );
        assert_eq!(resolve(&paths, "~/outside.sh", None), None);
        assert_eq!(resolve(&paths, "~/.claude/hooks/absent.sh", None), None);
    }
}
