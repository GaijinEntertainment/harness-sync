use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::atomic;
use crate::item::{ItemId, Items, KeyPath};

const PLACEHOLDER_PREFIX: &str = "<cc-sync secret ";

#[derive(Debug, Default, Serialize, Deserialize)]
struct MissingFile {
    missing: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Restored {
    pub(crate) value: Option<Value>,
    pub(crate) missing: Vec<KeyPath>,
}

pub(crate) fn placeholder(key: &KeyPath) -> String {
    format!("{PLACEHOLDER_PREFIX}{key}>")
}

pub(crate) fn is_placeholder(text: &str) -> bool {
    text.starts_with(PLACEHOLDER_PREFIX) && text.ends_with('>')
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    key == "AUTHORIZATION"
        || key.ends_with("_KEY")
        || ["TOKEN", "SECRET", "PASSWORD", "HEADERS"]
            .iter()
            .any(|part| key.contains(part))
}

pub(crate) fn local_view(setup: &Items, missing: &BTreeSet<KeyPath>) -> Items {
    let mut view: Items = setup
        .iter()
        .map(|(id, value)| {
            let mut value = value.clone();
            let secret = id.path.iter().any(|segment| is_secret_key(segment));
            mask(&mut value, &mut id.key_path(), secret);
            (id.clone(), value)
        })
        .collect();
    for key in missing {
        fill(&mut view, key);
    }
    view
}

fn mask(value: &mut Value, key: &mut KeyPath, secret: bool) {
    match value {
        Value::String(text) if secret => *text = placeholder(key),
        Value::Object(entries) => {
            for (name, child) in entries {
                key.path.push(name.clone());
                mask(child, key, secret || is_secret_key(name));
                key.path.pop();
            }
        }
        Value::Array(elements) => {
            for (index, child) in elements.iter_mut().enumerate() {
                key.path.push(index.to_string());
                mask(child, key, secret);
                key.path.pop();
            }
        }
        _ => {}
    }
}

fn fill(view: &mut Items, key: &KeyPath) {
    let item = key.item();
    let text = Value::String(placeholder(key));
    match (view.get_mut(&item), &key.path[item.path.len()..]) {
        (None, []) => {
            view.insert(item, text);
        }
        (Some(value), [parents @ .., name]) => {
            if let Some(Value::Object(parent)) = value_at_mut(value, parents) {
                parent.entry(name.clone()).or_insert(text);
            }
        }
        _ => {}
    }
}

pub(crate) fn restore(id: &ItemId, remote: &Value, local: Option<&Value>) -> Restored {
    let mut value = remote.clone();
    let mut missing = Vec::new();
    let keep = restore_value(&mut value, local, &mut id.key_path(), &mut missing);
    Restored {
        value: keep.then_some(value),
        missing,
    }
}

fn restore_value(
    value: &mut Value,
    local: Option<&Value>,
    key: &mut KeyPath,
    missing: &mut Vec<KeyPath>,
) -> bool {
    match value {
        Value::String(text) if is_placeholder(text) => {
            if let Some(local) = local {
                local.clone_into(value);
                return true;
            }
            missing.push(key.clone());
            false
        }
        Value::Object(entries) => {
            entries.retain(|name, child| {
                key.path.push(name.clone());
                let keep = restore_value(child, local.and_then(|l| l.get(name)), key, missing);
                key.path.pop();
                keep
            });
            true
        }
        Value::Array(elements) => {
            let mut index = 0;
            elements.retain_mut(|child| {
                key.path.push(index.to_string());
                let keep = restore_value(child, local.and_then(|l| l.get(index)), key, missing);
                key.path.pop();
                index += 1;
                keep
            });
            true
        }
        _ => true,
    }
}

pub(crate) fn still_missing(
    missing: impl IntoIterator<Item = KeyPath>,
    setup: &Items,
    remote: &Items,
) -> BTreeSet<KeyPath> {
    missing
        .into_iter()
        .filter(|key| {
            value_at_key(setup, key).is_none()
                && value_at_key(remote, key)
                    .and_then(Value::as_str)
                    .is_some_and(is_placeholder)
        })
        .collect()
}

fn value_at_key<'a>(items: &'a Items, key: &KeyPath) -> Option<&'a Value> {
    let item = key.item();
    key.path[item.path.len()..]
        .iter()
        .try_fold(items.get(&item)?, |value, segment| child(value, segment))
}

fn value_at_mut<'a>(value: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    path.iter().try_fold(value, |value, segment| match value {
        Value::Object(entries) => entries.get_mut(segment),
        Value::Array(elements) => elements.get_mut(segment.parse::<usize>().ok()?),
        _ => None,
    })
}

fn child<'a>(value: &'a Value, segment: &str) -> Option<&'a Value> {
    match value {
        Value::Object(entries) => entries.get(segment),
        Value::Array(elements) => elements.get(segment.parse::<usize>().ok()?),
        _ => None,
    }
}

pub(crate) fn load_missing(path: &Path) -> Result<BTreeSet<KeyPath>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let file: MissingFile =
        toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;
    file.missing
        .iter()
        .map(|key| key.parse())
        .collect::<Result<_>>()
        .with_context(|| format!("cannot parse {}", path.display()))
}

pub(crate) fn save_missing(path: &Path, missing: &BTreeSet<KeyPath>) -> Result<()> {
    let file = MissingFile {
        missing: missing.iter().map(ToString::to_string).collect(),
    };
    let text = toml::to_string(&file).context("cannot serialize the list of missing secrets")?;
    atomic::write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::item::settings_items;

    fn key(text: &str) -> KeyPath {
        text.parse().unwrap()
    }

    #[test]
    fn name_rules_match_secret_keys_in_any_case() {
        let secret = [
            "Authorization",
            "GITHUB_TOKEN",
            "api_key",
            "CLIENT_SECRET",
            "dbPassword",
            "headers",
        ];
        let plain = ["model", "apiKeyHelper", "KEYBOARD", "env"];

        assert!(secret.iter().all(|key| is_secret_key(key)), "{secret:?}");
        assert!(!plain.iter().any(|key| is_secret_key(key)), "{plain:?}");
    }

    #[test]
    fn the_local_view_replaces_each_secret_string_with_its_placeholder() {
        let setup = settings_items(json!({
            "env": {"GITHUB_TOKEN": "ghp_x", "EDITOR": "vim"},
            "pluginConfigs": {"p": {"options": {"headers": {"X-Id": "abc"}, "port": 1}}}
        }))
        .unwrap();

        let view = settings_items(json!({
            "env": {
                "GITHUB_TOKEN": "<cc-sync secret settings:/env/GITHUB_TOKEN>",
                "EDITOR": "vim"
            },
            "pluginConfigs": {"p": {"options": {
                "headers": {"X-Id": "<cc-sync secret settings:/pluginConfigs/p/options/headers/X-Id>"},
                "port": 1
            }}}
        }))
        .unwrap();
        assert_eq!(local_view(&setup, &BTreeSet::new()), view);
    }

    #[test]
    fn a_missing_secret_shows_its_placeholder_in_the_local_view() {
        let setup = settings_items(json!({"pluginConfigs": {"p": {"options": {}}}})).unwrap();
        let missing = BTreeSet::from([
            key("settings:/env/GITHUB_TOKEN"),
            key("settings:/pluginConfigs/p/options/apiToken"),
        ]);

        let view = local_view(&setup, &missing);

        assert_eq!(
            view,
            settings_items(json!({
                "env": {"GITHUB_TOKEN": "<cc-sync secret settings:/env/GITHUB_TOKEN>"},
                "pluginConfigs": {"p": {"options": {
                    "apiToken": "<cc-sync secret settings:/pluginConfigs/p/options/apiToken>"
                }}}
            }))
            .unwrap()
        );
    }

    #[test]
    fn restore_takes_each_secret_from_the_local_value() {
        let id = key("settings:/pluginConfigs/p").item();
        let remote =
            json!({"url": "b", "apiToken": "<cc-sync secret settings:/pluginConfigs/p/apiToken>"});
        let local = json!({"url": "a", "apiToken": "mine"});

        let restored = restore(&id, &remote, Some(&local));

        assert_eq!(
            restored.value,
            Some(json!({"url": "b", "apiToken": "mine"}))
        );
        assert_eq!(restored.missing, []);
    }

    #[test]
    fn restore_leaves_out_a_secret_that_the_machine_does_not_have() {
        let id = key("settings:/pluginConfigs/p").item();
        let remote =
            json!({"url": "b", "apiToken": "<cc-sync secret settings:/pluginConfigs/p/apiToken>"});
        let whole = key("settings:/env/GITHUB_TOKEN").item();

        let partial = restore(&id, &remote, None);
        let absent = restore(
            &whole,
            &json!("<cc-sync secret settings:/env/GITHUB_TOKEN>"),
            None,
        );

        assert_eq!(partial.value, Some(json!({"url": "b"})));
        assert_eq!(partial.missing, [key("settings:/pluginConfigs/p/apiToken")]);
        assert_eq!(absent.value, None);
        assert_eq!(absent.missing, [key("settings:/env/GITHUB_TOKEN")]);
    }

    #[test]
    fn a_secret_stays_missing_until_the_machine_has_a_value_or_the_remote_drops_it() {
        let set = key("settings:/env/SET_TOKEN");
        let dropped = key("settings:/env/DROPPED_TOKEN");
        let open = key("settings:/env/OPEN_TOKEN");
        let setup = settings_items(json!({"env": {"SET_TOKEN": "x"}})).unwrap();
        let remote = settings_items(json!({"env": {
            "SET_TOKEN": "<cc-sync secret settings:/env/SET_TOKEN>",
            "OPEN_TOKEN": "<cc-sync secret settings:/env/OPEN_TOKEN>"
        }}))
        .unwrap();

        let missing = still_missing([set, dropped, open.clone()], &setup, &remote);

        assert_eq!(missing, BTreeSet::from([open]));
    }
}
