use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Result, bail};
use serde_json::{Map, Value};

const SPLIT_SETTINGS_KEYS: [&str; 6] = [
    "env",
    "enabledPlugins",
    "extraKnownMarketplaces",
    "pluginConfigs",
    "skillOverrides",
    "modelSettings",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    Settings,
}

impl Source {
    const fn name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ItemId {
    pub(crate) source: Source,
    pub(crate) path: Vec<String>,
}

pub(crate) type Items = BTreeMap<ItemId, Value>;

impl ItemId {
    fn settings(path: &[&str]) -> Self {
        Self {
            source: Source::Settings,
            path: path.iter().map(|&segment| segment.to_owned()).collect(),
        }
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.source.name())?;
        for segment in &self.path {
            write!(f, "/{}", segment.replace('~', "~0").replace('/', "~1"))?;
        }
        Ok(())
    }
}

pub(crate) fn settings_items(document: Value) -> Result<Items> {
    let Value::Object(keys) = document else {
        bail!("settings.json does not hold a JSON object");
    };
    let mut items = Items::new();
    for (key, value) in keys {
        match value {
            Value::Object(entries) if SPLIT_SETTINGS_KEYS.contains(&key.as_str()) => {
                for (name, entry) in entries {
                    items.insert(ItemId::settings(&[&key, &name]), entry);
                }
            }
            value => {
                items.insert(ItemId::settings(&[&key]), value);
            }
        }
    }
    Ok(items)
}

pub(crate) fn settings_document(items: &Items) -> Value {
    let mut document = Map::new();
    for (id, value) in items {
        match id.source {
            Source::Settings => insert_at(&mut document, &id.path, value.clone()),
        }
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
                "settings:/effortLevel",
                "settings:/env/A",
                "settings:/env/B",
                "settings:/permissions"
            ]
        );
        assert_eq!(
            items[&ItemId::settings(&["permissions"])],
            json!({"deny": ["Bash"]})
        );
    }

    #[test]
    fn a_split_key_without_an_object_is_one_item() {
        let items = settings_items(json!({"env": "broken"})).unwrap();

        assert_eq!(items[&ItemId::settings(&["env"])], json!("broken"));
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

        let rebuilt = settings_document(&items);

        assert_eq!(rebuilt, document);
        assert_eq!(settings_items(rebuilt).unwrap(), items);
    }

    #[test]
    fn item_ids_escape_json_pointer_segments() {
        let id = ItemId::settings(&["skillOverrides", "plugin:skill/a~b"]);

        assert_eq!(
            id.to_string(),
            "settings:/skillOverrides/plugin:skill~1a~0b"
        );
    }
}
