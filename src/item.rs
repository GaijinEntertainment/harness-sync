use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value};

const SPLIT_SETTINGS_KEYS: [&str; 6] = [
    "env",
    "enabledPlugins",
    "extraKnownMarketplaces",
    "pluginConfigs",
    "skillOverrides",
    "modelSettings",
];

const CLAUDE_KEYS: [&str; 30] = [
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
    "autoUpdates",
    "showSpinnerTree",
    "diffSidebarOpen",
    "deepLinkTerminal",
    "lspRecommendationDisabled",
    "companionMuted",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum JsonFile {
    Settings,
    Claude,
}

pub(crate) const JSON_SOURCES: [JsonFile; 2] = [JsonFile::Settings, JsonFile::Claude];

impl JsonFile {
    const fn name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Claude => "claude",
        }
    }

    pub(crate) const fn snapshot_file(self) -> &'static str {
        match self {
            Self::Settings => "settings.json",
            Self::Claude => "claude.json",
        }
    }

    const fn split_keys(self) -> &'static [&'static str] {
        match self {
            Self::Settings => &SPLIT_SETTINGS_KEYS,
            Self::Claude => &["mcpServers"],
        }
    }

    fn splits(self, key: &str) -> bool {
        self.split_keys().contains(&key)
    }

    fn holds(self, key: &str) -> bool {
        match self {
            Self::Settings => true,
            Self::Claude => CLAUDE_KEYS.contains(&key),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    Json(JsonFile),
    File,
}

impl Source {
    const fn name(self) -> &'static str {
        match self {
            Self::Json(file) => file.name(),
            Self::File => "file",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        JSON_SOURCES
            .into_iter()
            .map(Self::Json)
            .find(|source| source.name() == name)
    }

    fn item_length(self, path: &[String]) -> usize {
        match self {
            Self::Json(file) => {
                if path.first().is_some_and(|key| file.splits(key)) {
                    2
                } else {
                    1
                }
            }
            Self::File => path.len(),
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
    fn new(source: Source, path: &[&str]) -> Self {
        Self {
            source,
            path: path.iter().map(|&segment| segment.to_owned()).collect(),
        }
    }

    #[cfg(test)]
    fn settings(path: &[&str]) -> Self {
        Self::new(Source::Json(JsonFile::Settings), path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct KeyPath {
    pub(crate) source: Source,
    pub(crate) path: Vec<String>,
}

impl ItemId {
    pub(crate) fn key_path(&self) -> KeyPath {
        KeyPath {
            source: self.source,
            path: self.path.clone(),
        }
    }
}

impl ItemId {
    pub(crate) fn names_an_item(&self) -> bool {
        !self.path.is_empty() && self.path.len() == self.source.item_length(&self.path)
    }
}

impl KeyPath {
    pub(crate) fn item(&self) -> ItemId {
        let length = self.source.item_length(&self.path);
        ItemId {
            source: self.source,
            path: self.path.iter().take(length).cloned().collect(),
        }
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_pointer(f, self.source, &self.path)
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_pointer(f, self.source, &self.path)
    }
}

impl FromStr for ItemId {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (source, path) = parse_pointer(text)?;
        Ok(Self { source, path })
    }
}

impl FromStr for KeyPath {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (source, path) = parse_pointer(text)?;
        Ok(Self { source, path })
    }
}

fn write_pointer(f: &mut fmt::Formatter<'_>, source: Source, path: &[String]) -> fmt::Result {
    write!(f, "{}:", source.name())?;
    if source == Source::File {
        return write!(f, "{}", path.join("/"));
    }
    for segment in path {
        write!(f, "/{}", segment.replace('~', "~0").replace('/', "~1"))?;
    }
    Ok(())
}

fn parse_pointer(text: &str) -> Result<(Source, Vec<String>)> {
    if let Some(relative) = text.strip_prefix("file:") {
        let path: Vec<String> = relative.split('/').map(str::to_owned).collect();
        ensure!(
            path.iter()
                .all(|segment| !matches!(segment.as_str(), "" | "." | "..")),
            "`{text}` is not a file item id; a file item id looks like `file:rules/NAME.md`"
        );
        return Ok((Source::File, path));
    }
    let Some((source, pointer)) = text
        .split_once(':')
        .and_then(|(name, pointer)| Some((Source::from_name(name)?, pointer)))
    else {
        bail!("`{text}` is not an item id; an item id looks like `settings:/env/NAME`");
    };
    let Some(pointer) = pointer.strip_prefix('/') else {
        bail!("the item id `{text}` needs a JSON pointer that starts with `/`");
    };
    let path = pointer
        .split('/')
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect();
    Ok((source, path))
}

pub(crate) fn json_items(file: JsonFile, document: Value, include: &[KeyPath]) -> Result<Items> {
    let source = Source::Json(file);
    let Value::Object(keys) = document else {
        bail!("the document does not hold a JSON object");
    };
    let included = |key: &str| {
        include.iter().any(|path| {
            path.source == source && path.path.first().is_some_and(|first| first == key)
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
                    items.insert(ItemId::new(source, &[&key, &name]), entry);
                }
            }
            value => {
                items.insert(ItemId::new(source, &[&key]), value);
            }
        }
    }
    Ok(items)
}

#[cfg(test)]
pub(crate) fn settings_items(document: Value) -> Result<Items> {
    json_items(JsonFile::Settings, document, &[])
}

pub(crate) fn json_document(items: &Items, file: JsonFile) -> Value {
    let mut document = Map::new();
    for (id, value) in items
        .iter()
        .filter(|(id, _)| id.source == Source::Json(file))
    {
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

    #[test]
    fn claude_json_gives_the_catalog_and_included_keys_and_one_item_per_mcp_server() {
        let items = json_items(
            JsonFile::Claude,
            json!({
                "numStartups": 12,
                "oauthAccount": {"emailAddress": "me@example.com"},
                "projects": {"/x": {"mcpServers": {"p": {}}}},
                "mcpServers": {"kartta": {"command": "kartta"}, "web": {"type": "http", "url": "u"}},
                "theme": "dark",
                "autoUpdates": false
            }),
            &["claude:/numStartups".parse().unwrap()],
        )
        .unwrap();

        let ids: Vec<String> = items.keys().map(ToString::to_string).collect();
        assert_eq!(
            ids,
            [
                "claude:/autoUpdates",
                "claude:/mcpServers/kartta",
                "claude:/mcpServers/web",
                "claude:/numStartups",
                "claude:/theme"
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

        let rebuilt = json_document(&items, JsonFile::Settings);

        assert_eq!(rebuilt, document);
        assert_eq!(settings_items(rebuilt).unwrap(), items);
    }

    #[test]
    fn item_ids_escape_and_parse_json_pointer_segments() {
        let id = ItemId::settings(&["skillOverrides", "plugin:skill/a~b"]);

        let text = id.to_string();

        assert_eq!(text, "settings:/skillOverrides/plugin:skill~1a~0b");
        assert_eq!(text.parse::<ItemId>().unwrap(), id);
    }

    #[test]
    fn a_key_path_belongs_to_the_item_that_holds_it() {
        let in_split_key: KeyPath = "settings:/pluginConfigs/p/options/apiToken"
            .parse()
            .unwrap();
        let in_plain_key: KeyPath = "settings:/statusLine/command".parse().unwrap();

        assert_eq!(
            in_split_key.item(),
            ItemId::settings(&["pluginConfigs", "p"])
        );
        assert_eq!(in_plain_key.item(), ItemId::settings(&["statusLine"]));
    }

    #[test]
    fn a_file_item_id_is_its_relative_path() {
        let id: ItemId = "file:skills/review/SKILL.md".parse().unwrap();

        assert_eq!(id.source, Source::File);
        assert_eq!(id.path, ["skills", "review", "SKILL.md"]);
        assert_eq!(id.to_string(), "file:skills/review/SKILL.md");
        assert!(id.names_an_item());
        assert!("file:../x".parse::<ItemId>().is_err());
        assert!("file:a//b".parse::<ItemId>().is_err());
    }

    #[test]
    fn a_text_that_is_not_an_item_id_is_rejected() {
        assert!("files:/x".parse::<ItemId>().is_err());
        assert!("settings:env".parse::<ItemId>().is_err());
    }
}
