//! Item ids and key paths: a path relative to the home directory, plus `:` and a JSON pointer for a key of a JSON
//! file, such as `.claude/settings.json:/env/EDITOR` (ADR 1.14).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value};

use crate::harness::{KeyedFile, keyed_file};

/// A supported item: a whole file, or one key of a keyed JSON file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ItemId {
    /// The path of the file, relative to the home directory.
    pub file: String,
    /// The keys from the root of the JSON document to the item, or none for a whole file.
    pub path: Vec<String>,
}

/// Supported items with their values.
pub type Items = BTreeMap<ItemId, Value>;

impl ItemId {
    pub(crate) fn whole_file(file: &str) -> Self {
        Self {
            file: file.to_owned(),
            path: Vec::new(),
        }
    }

    /// The item at the keys `path` of the JSON file `file`.
    #[must_use]
    pub fn key(file: &str, path: &[&str]) -> Self {
        Self {
            file: file.to_owned(),
            path: path.iter().map(|&segment| segment.to_owned()).collect(),
        }
    }

    pub(crate) const fn is_whole_file(&self) -> bool {
        self.path.is_empty()
    }

    /// The key path of the item.
    #[must_use]
    pub fn key_path(&self) -> KeyPath {
        KeyPath {
            file: self.file.clone(),
            path: self.path.clone(),
        }
    }

    pub(crate) fn names_an_item(&self) -> bool {
        keyed_file(&self.file).map_or(self.path.is_empty(), |keyed| {
            !self.path.is_empty() && self.path.len() == keyed.item_length(&self.path)
        })
    }
}

/// A file, a directory, or a key at any depth of a JSON file: what the repo's lists, a secret, and a command name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyPath {
    /// The path of the file or directory, relative to the home directory.
    pub file: String,
    /// The keys from the root of the JSON document, or none for the whole file or directory.
    pub path: Vec<String>,
}

impl KeyPath {
    pub(crate) fn item(&self) -> ItemId {
        let length = keyed_file(&self.file).map_or(0, |keyed| keyed.item_length(&self.path));
        ItemId {
            file: self.file.clone(),
            path: self.path.iter().take(length).cloned().collect(),
        }
    }

    pub(crate) fn covers(&self, file: &str, path: &[String]) -> bool {
        if self.path.is_empty() {
            is_within(file, &self.file)
        } else {
            file == self.file && path.starts_with(&self.path)
        }
    }
}

pub(crate) fn is_within(file: &str, dir: &str) -> bool {
    file == dir
        || file
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_id(f, &self.file, &self.path)
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_id(f, &self.file, &self.path)
    }
}

impl FromStr for ItemId {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (file, path) = parse_id(text)?;
        Ok(Self { file, path })
    }
}

impl FromStr for KeyPath {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (file, path) = parse_id(text)?;
        Ok(Self { file, path })
    }
}

fn write_id(f: &mut fmt::Formatter<'_>, file: &str, path: &[String]) -> fmt::Result {
    f.write_str(file)?;
    if !path.is_empty() {
        f.write_str(":")?;
    }
    for segment in path {
        write!(f, "/{}", segment.replace('~', "~0").replace('/', "~1"))?;
    }
    Ok(())
}

fn parse_id(text: &str) -> Result<(String, Vec<String>)> {
    let pointer_start = text.match_indices(":/").map(|(at, _)| at).find(|&at| {
        Path::new(&text[..at])
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    });
    let (file, pointer) =
        pointer_start.map_or((text, None), |at| (&text[..at], Some(&text[at + 2..])));
    ensure!(
        is_home_path(file),
        "`{text}` is not an item id; an item id is a path relative to the home directory, such as \
         `.claude/rules/NAME.md`, and for a JSON key the path, `:`, and a JSON pointer, such as \
         `.claude/settings.json:/env/NAME`"
    );
    let path = pointer.map_or_else(Vec::new, |pointer| {
        pointer
            .split('/')
            .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
            .collect()
    });
    Ok((file.to_owned(), path))
}

fn is_home_path(file: &str) -> bool {
    !file.contains('\\')
        && !file.starts_with('~')
        && file
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

pub(crate) fn json_items(file: &KeyedFile, document: Value, include: &[KeyPath]) -> Result<Items> {
    let Value::Object(keys) = document else {
        bail!("the document does not hold a JSON object");
    };
    let included = |key: &str| {
        include.iter().any(|path| {
            path.file == file.path && path.path.first().is_some_and(|first| first == key)
        })
    };
    let mut items = Items::new();
    for (key, value) in keys
        .into_iter()
        .filter(|(key, _)| file.holds(key) || included(key))
    {
        match value {
            Value::Object(entries) if file.splits(&key) => {
                for (name, entry) in entries {
                    items.insert(ItemId::key(file.path, &[&key, &name]), entry);
                }
            }
            value => {
                items.insert(ItemId::key(file.path, &[&key]), value);
            }
        }
    }
    Ok(items)
}

#[cfg(test)]
pub(crate) fn settings_items(document: Value) -> Result<Items> {
    let settings = keyed_file(".claude/settings.json").expect("Claude Code has settings.json");
    json_items(settings, document, &[])
}

pub(crate) fn json_document(items: &Items, file: &str) -> Value {
    let mut document = Map::new();
    for (id, value) in items.iter().filter(|(id, _)| id.file == file) {
        insert_at(&mut document, &id.path, value.clone());
    }
    Value::Object(document)
}

fn insert_at(object: &mut Map<String, Value>, path: &[String], value: Value) {
    match path {
        [] => {}
        [key] => {
            object.insert(key.clone(), value);
        }
        [key, rest @ ..] => {
            let child = object
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if !child.is_object() {
                *child = Value::Object(Map::new());
            }
            if let Value::Object(child) = child {
                insert_at(child, rest, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const CLAUDE_SETTINGS: &str = ".claude/settings.json";

    #[test]
    fn claude_json_gives_the_catalog_and_included_keys_and_one_item_per_mcp_server() {
        let items = json_items(
            keyed_file(".claude.json").unwrap(),
            json!({
                "numStartups": 12,
                "oauthAccount": {"emailAddress": "me@example.com"},
                "projects": {"/x": {"mcpServers": {"p": {}}}},
                "mcpServers": {"kartta": {"command": "kartta"}, "web": {"type": "http", "url": "u"}},
                "theme": "dark",
                "autoUpdates": false
            }),
            &[".claude.json:/numStartups".parse().unwrap()],
        )
        .unwrap();

        let ids: Vec<String> = items.keys().map(ToString::to_string).collect();
        assert_eq!(
            ids,
            [
                ".claude.json:/mcpServers/kartta",
                ".claude.json:/mcpServers/web",
                ".claude.json:/numStartups",
                ".claude.json:/theme"
            ]
        );
    }

    #[test]
    fn split_keys_become_one_item_per_entry() {
        let items = settings_items(json!({
            "env": {"A": "1", "B": "2"},
            "permissions": {"deny": ["Bash"]},
            "effortLevel": "high"
        }))
        .unwrap();

        let ids: Vec<String> = items.keys().map(ToString::to_string).collect();
        assert_eq!(
            ids,
            [
                ".claude/settings.json:/effortLevel",
                ".claude/settings.json:/env/A",
                ".claude/settings.json:/env/B",
                ".claude/settings.json:/permissions"
            ]
        );
        assert_eq!(
            items[&ItemId::key(CLAUDE_SETTINGS, &["permissions"])],
            json!({"deny": ["Bash"]})
        );
    }

    #[test]
    fn a_split_key_without_an_object_is_one_item() {
        let items = settings_items(json!({"env": "broken"})).unwrap();

        assert_eq!(
            items[&ItemId::key(CLAUDE_SETTINGS, &["env"])],
            json!("broken")
        );
    }

    #[test]
    fn a_document_that_is_not_an_object_is_rejected() {
        assert!(settings_items(json!([1, 2])).is_err());
    }

    #[test]
    fn a_settings_document_holds_its_items_again() {
        let document =
            json!({"env": {"A": "1"}, "permissions": {"deny": ["Bash"]}, "model": "opus"});
        let items = settings_items(document.clone()).unwrap();

        let rebuilt = json_document(&items, CLAUDE_SETTINGS);

        assert_eq!(rebuilt, document);
        assert_eq!(settings_items(rebuilt).unwrap(), items);
    }

    #[test]
    fn item_ids_escape_and_parse_json_pointer_segments() {
        let id = ItemId::key(CLAUDE_SETTINGS, &["skillOverrides", "plugin:skill/a~b"]);

        let text = id.to_string();

        assert_eq!(
            text,
            ".claude/settings.json:/skillOverrides/plugin:skill~1a~0b"
        );
        assert_eq!(text.parse::<ItemId>().unwrap(), id);
    }

    #[test]
    fn a_key_path_belongs_to_the_item_that_holds_it() {
        let in_split_key: KeyPath = ".claude/settings.json:/pluginConfigs/p/options/apiToken"
            .parse()
            .unwrap();
        let in_plain_key: KeyPath = ".claude/settings.json:/statusLine/command".parse().unwrap();
        let in_a_file: KeyPath = ".claude/keybindings.json:/bindings".parse().unwrap();

        assert_eq!(
            in_split_key.item(),
            ItemId::key(CLAUDE_SETTINGS, &["pluginConfigs", "p"])
        );
        assert_eq!(
            in_plain_key.item(),
            ItemId::key(CLAUDE_SETTINGS, &["statusLine"])
        );
        assert_eq!(
            in_a_file.item(),
            ItemId::whole_file(".claude/keybindings.json")
        );
    }

    #[test]
    fn a_file_item_id_is_its_home_relative_path() {
        let id: ItemId = ".claude/skills/review/SKILL.md".parse().unwrap();

        assert_eq!(id, ItemId::whole_file(".claude/skills/review/SKILL.md"));
        assert_eq!(id.to_string(), ".claude/skills/review/SKILL.md");
        assert!(id.names_an_item());
        assert!(
            ".claude/skills/a:/b.md"
                .parse::<ItemId>()
                .unwrap()
                .is_whole_file()
        );
    }

    #[test]
    fn only_an_item_of_its_file_kind_names_an_item() {
        let names = |text: &str| text.parse::<ItemId>().unwrap().names_an_item();

        assert!(names(".claude/settings.json:/env/A"));
        assert!(!names(".claude/settings.json:/env"));
        assert!(!names(".claude/settings.json"));
        assert!(!names(".claude/keybindings.json:/x"));
    }

    #[test]
    fn a_location_covers_its_keys_and_the_files_below_it() {
        let dir: KeyPath = ".claude/skills/review".parse().unwrap();
        let key: KeyPath = ".claude/settings.json:/env".parse().unwrap();

        assert!(dir.covers(".claude/skills/review/SKILL.md", &[]));
        assert!(!dir.covers(".claude/skills/reviewer/SKILL.md", &[]));
        assert!(key.covers(CLAUDE_SETTINGS, &["env".to_owned(), "A".to_owned()]));
        assert!(!key.covers(CLAUDE_SETTINGS, &["envy".to_owned()]));
    }

    #[test]
    fn a_text_that_is_not_an_item_id_is_rejected() {
        for text in [
            "",
            "~/.claude/settings.json",
            "/etc/passwd",
            ".claude/../x",
            ".claude//x",
            ".claude\\x",
        ] {
            assert!(text.parse::<ItemId>().is_err(), "{text}");
        }
    }
}
