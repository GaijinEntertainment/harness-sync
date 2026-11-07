//! The secret masking of push, the secret restore of pull, and the secret scan (ADR 1.2).

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::atomic;
use crate::item::{ItemId, Items, KeyPath};

const PLACEHOLDER_PREFIX: &str = "<harness-sync secret ";

const TOKEN_PREFIXES: [(&str, usize); 17] = [
    ("sk-ant-", 20),
    ("sk-proj-", 20),
    ("ghp_", 30),
    ("gho_", 30),
    ("ghu_", 30),
    ("ghs_", 30),
    ("ghr_", 30),
    ("github_pat_", 30),
    ("glpat-", 20),
    ("xoxb-", 20),
    ("xoxp-", 20),
    ("AKIA", 16),
    ("AIza", 30),
    ("npm_", 30),
    ("hf_", 30),
    ("pypi-", 30),
    ("Bearer ", 20),
];

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
        || [
            "_KEY", "-KEY", "APIKEY", "TOKEN", "SECRET", "PASSWORD", "HEADERS",
        ]
        .iter()
        .any(|ending| key.ends_with(ending))
}

pub(crate) fn local_view(
    setup: &Items,
    missing: &BTreeSet<KeyPath>,
    declared: &[KeyPath],
) -> Items {
    let mut view: Items = setup
        .iter()
        .map(|(id, value)| {
            let mut value = value.clone();
            if !id.is_whole_file() {
                let mut key = id.key_path();
                let secret = is_secret_path(&key, declared);
                mask(&mut value, &mut key, secret, declared);
            }
            (id.clone(), value)
        })
        .collect();
    for key in missing {
        fill(&mut view, key);
    }
    view
}

fn mask(value: &mut Value, key: &mut KeyPath, secret: bool, declared: &[KeyPath]) {
    match value {
        Value::String(text) if secret => *text = placeholder(key),
        Value::Object(entries) => {
            for (name, child) in entries {
                key.path.push(name.clone());
                let secret = secret || is_secret_key(name) || is_declared(key, declared);
                mask(child, key, secret, declared);
                key.path.pop();
            }
        }
        Value::Array(elements) => {
            for (index, child) in elements.iter_mut().enumerate() {
                key.path.push(index.to_string());
                let secret = secret || is_declared(key, declared);
                mask(child, key, secret, declared);
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

pub(crate) type Ask<'a> = dyn FnMut(&KeyPath) -> Result<Option<String>> + 'a;

pub(crate) fn restore(
    id: &ItemId,
    remote: &Value,
    local: Option<&Value>,
    ask: &mut Ask<'_>,
) -> Result<Restored> {
    let mut value = remote.clone();
    let mut restore = Restore {
        key: id.key_path(),
        missing: Vec::new(),
        ask,
        error: None,
    };
    let keep = restore.value(&mut value, local);
    if let Some(err) = restore.error {
        return Err(err);
    }
    Ok(Restored {
        value: keep.then_some(value),
        missing: restore.missing,
    })
}

struct Restore<'a, 'b> {
    key: KeyPath,
    missing: Vec<KeyPath>,
    ask: &'a mut Ask<'b>,
    error: Option<anyhow::Error>,
}

impl Restore<'_, '_> {
    fn value(&mut self, value: &mut Value, local: Option<&Value>) -> bool {
        match value {
            Value::String(text) if is_placeholder(text) => self.secret(value, local),
            Value::Object(entries) => {
                entries.retain(|name, child| {
                    self.key.path.push(name.clone());
                    let keep = self.value(child, local.and_then(|l| l.get(name)));
                    self.key.path.pop();
                    keep
                });
                true
            }
            Value::Array(elements) => {
                let mut index = 0;
                elements.retain_mut(|child| {
                    self.key.path.push(index.to_string());
                    let keep = self.value(child, local.and_then(|l| l.get(index)));
                    self.key.path.pop();
                    index += 1;
                    keep
                });
                true
            }
            _ => true,
        }
    }

    fn secret(&mut self, value: &mut Value, local: Option<&Value>) -> bool {
        if let Some(local) = local {
            local.clone_into(value);
            return true;
        }
        if self.error.is_some() {
            return false;
        }
        match (self.ask)(&self.key) {
            Ok(Some(entered)) => {
                *value = Value::String(entered);
                true
            }
            Ok(None) => {
                self.missing.push(self.key.clone());
                false
            }
            Err(err) => {
                self.error = Some(err);
                false
            }
        }
    }
}

pub(crate) fn is_secret_path(key: &KeyPath, declared: &[KeyPath]) -> bool {
    key.path.iter().any(|segment| is_secret_key(segment)) || is_declared(key, declared)
}

fn is_declared(key: &KeyPath, declared: &[KeyPath]) -> bool {
    declared
        .iter()
        .any(|path| path.covers(&key.file, &key.path))
}

pub(crate) fn scan(items: &Items) -> Vec<KeyPath> {
    let mut found = Vec::new();
    for (id, value) in items {
        scan_value(value, &mut id.key_path(), &mut found);
    }
    found
}

/// The value of the item `id` with a secret placeholder in place of each secret.
#[must_use]
pub fn redact(id: &ItemId, value: &Value) -> Value {
    let mut redacted = value.clone();
    let mut found = Vec::new();
    scan_value(value, &mut id.key_path(), &mut found);
    for key in found {
        if let Some(text) = value_at_mut(&mut redacted, &key.path[id.path.len()..]) {
            *text = Value::String(placeholder(&key));
        }
    }
    redacted
}

fn scan_value(value: &Value, key: &mut KeyPath, found: &mut Vec<KeyPath>) {
    match value {
        Value::String(text) if !is_placeholder(text) && looks_like_token(text) => {
            found.push(key.clone());
        }
        Value::Object(entries) => {
            for (name, child) in entries {
                key.path.push(name.clone());
                scan_value(child, key, found);
                key.path.pop();
            }
        }
        Value::Array(elements) => {
            for (index, child) in elements.iter().enumerate() {
                key.path.push(index.to_string());
                scan_value(child, key, found);
                key.path.pop();
            }
        }
        _ => {}
    }
}

pub(crate) fn looks_like_token(text: &str) -> bool {
    let is_token_char =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+' | '/' | '=');
    let prefixed = TOKEN_PREFIXES.iter().any(|&(prefix, length)| {
        text.match_indices(prefix).any(|(start, _)| {
            let at_boundary = text[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric());
            at_boundary
                && text[start + prefix.len()..]
                    .chars()
                    .take_while(|&c| is_token_char(c))
                    .count()
                    >= length
        })
    });
    prefixed || text.contains("PRIVATE KEY-----") || is_jwt(text)
}

fn is_jwt(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || c == '"')
        .any(|word| {
            let parts: Vec<&str> = word.split('.').collect();
            parts.len() == 3
                && parts[0].starts_with("eyJ")
                && parts[1].starts_with("eyJ")
                && parts[2].len() >= 10
        })
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
            "accessToken",
            "api_key",
            "CLIENT_SECRET",
            "dbPassword",
            "headers",
            "OTEL_EXPORTER_OTLP_HEADERS",
            "apiKey",
            "api-key",
            "X-API-KEY",
        ];
        let plain = ["model", "apiKeyHelper", "KEYBOARD", "env", "monkey"];

        assert!(secret.iter().all(|key| is_secret_key(key)), "{secret:?}");
        assert!(!plain.iter().any(|key| is_secret_key(key)), "{plain:?}");
    }

    #[test]
    fn name_rules_skip_keys_that_hold_a_secret_word_before_their_end() {
        let plain = [
            "CLAUDE_CODE_MAX_OUTPUT_TOKENS",
            "MAX_THINKING_TOKENS",
            "otelHeadersHelper",
            "headersHelper",
            "PASSWORD_FILE",
        ];

        assert!(!plain.iter().any(|key| is_secret_key(key)), "{plain:?}");
    }

    #[test]
    fn the_scan_finds_token_like_strings_outside_placeholders() {
        let tokens = [
            "ghp_0123456789abcdefghijABCDEFGHIJ012345",
            "Authorization: Bearer abcdefghijklmnopqrstuvwxyz",
            "x sk-ant-api03-abcdefghijklmnopqrstuvwxyz y",
            "AKIAABCDEFGHIJKLMNOP",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJlMTIz",
        ];
        let plain = [
            "ghp_short",
            "/Users/me/.claude/statusline.sh",
            "Bearer ${TOKEN}",
            "task-ant-0123456789abcdefghijklmnop",
            "<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>",
        ];

        assert!(
            tokens.iter().all(|text| looks_like_token(text)),
            "{tokens:?}"
        );
        assert!(
            !plain.iter().any(|text| looks_like_token(text)),
            "{plain:?}"
        );
    }

    #[test]
    fn the_scan_reports_key_paths_and_skips_placeholders() {
        let items = settings_items(json!({
            "env": {"NOTE": "ghp_0123456789abcdefghijABCDEFGHIJ012345", "GITHUB_TOKEN": "<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>"},
            "hooks": {"Stop": [{"command": "curl -H 'Bearer abcdefghijklmnopqrstuvwxyz'"}]}
        }))
        .unwrap();

        let found = scan(&items);

        assert_eq!(
            found,
            [
                key(".claude/settings.json:/env/NOTE"),
                key(".claude/settings.json:/hooks/Stop/0/command")
            ]
        );
    }

    #[test]
    fn redact_replaces_token_like_strings_with_their_placeholder() {
        let id = key(".claude/settings.json:/statusLine").item();
        let value =
            json!({"command": "ghp_0123456789abcdefghijABCDEFGHIJ012345", "type": "command"});

        let redacted = redact(&id, &value);

        assert_eq!(
            redacted,
            json!({"command": "<harness-sync secret .claude/settings.json:/statusLine/command>", "type": "command"})
        );
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
                "GITHUB_TOKEN": "<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>",
                "EDITOR": "vim"
            },
            "pluginConfigs": {"p": {"options": {
                "headers": {"X-Id": "<harness-sync secret .claude/settings.json:/pluginConfigs/p/options/headers/X-Id>"},
                "port": 1
            }}}
        }))
        .unwrap();
        assert_eq!(local_view(&setup, &BTreeSet::new(), &[]), view);
    }

    #[test]
    fn a_declared_key_path_makes_every_string_under_it_a_secret() {
        let setup = settings_items(json!({
            "env": {"INTERNAL_URL": "https://x"},
            "pluginConfigs": {"p": {"options": {"url": "u", "port": 1}, "name": "n"}}
        }))
        .unwrap();
        let declared = [
            key(".claude/settings.json:/env/INTERNAL_URL"),
            key(".claude/settings.json:/pluginConfigs/p/options"),
        ];

        let view = local_view(&setup, &BTreeSet::new(), &declared);

        assert_eq!(
            view,
            settings_items(json!({
                "env": {"INTERNAL_URL": "<harness-sync secret .claude/settings.json:/env/INTERNAL_URL>"},
                "pluginConfigs": {"p": {
                    "options": {"url": "<harness-sync secret .claude/settings.json:/pluginConfigs/p/options/url>", "port": 1},
                    "name": "n"
                }}
            }))
            .unwrap()
        );
    }

    #[test]
    fn a_missing_secret_shows_its_placeholder_in_the_local_view() {
        let setup = settings_items(json!({"pluginConfigs": {"p": {"options": {}}}})).unwrap();
        let missing = BTreeSet::from([
            key(".claude/settings.json:/env/GITHUB_TOKEN"),
            key(".claude/settings.json:/pluginConfigs/p/options/apiToken"),
        ]);

        let view = local_view(&setup, &missing, &[]);

        assert_eq!(
            view,
            settings_items(json!({
                "env": {"GITHUB_TOKEN": "<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>"},
                "pluginConfigs": {"p": {"options": {
                    "apiToken": "<harness-sync secret .claude/settings.json:/pluginConfigs/p/options/apiToken>"
                }}}
            }))
            .unwrap()
        );
    }

    #[test]
    fn restore_takes_each_secret_from_the_local_value() {
        let id = key(".claude/settings.json:/pluginConfigs/p").item();
        let remote = json!({"url": "b", "apiToken": "<harness-sync secret .claude/settings.json:/pluginConfigs/p/apiToken>"});
        let local = json!({"url": "a", "apiToken": "mine"});

        let restored = restore(&id, &remote, Some(&local), &mut |_| Ok(None)).unwrap();

        assert_eq!(
            restored.value,
            Some(json!({"url": "b", "apiToken": "mine"}))
        );
        assert_eq!(restored.missing, []);
    }

    #[test]
    fn restore_leaves_out_a_secret_that_the_machine_does_not_have() {
        let id = key(".claude/settings.json:/pluginConfigs/p").item();
        let remote = json!({"url": "b", "apiToken": "<harness-sync secret .claude/settings.json:/pluginConfigs/p/apiToken>"});
        let whole = key(".claude/settings.json:/env/GITHUB_TOKEN").item();

        let partial = restore(&id, &remote, None, &mut |_| Ok(None)).unwrap();
        let absent = restore(
            &whole,
            &json!("<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>"),
            None,
            &mut |_| Ok(None),
        )
        .unwrap();

        assert_eq!(partial.value, Some(json!({"url": "b"})));
        assert_eq!(
            partial.missing,
            [key(".claude/settings.json:/pluginConfigs/p/apiToken")]
        );
        assert_eq!(absent.value, None);
        assert_eq!(
            absent.missing,
            [key(".claude/settings.json:/env/GITHUB_TOKEN")]
        );
    }

    #[test]
    fn restore_asks_for_a_secret_that_the_machine_does_not_have() {
        let id = key(".claude/settings.json:/env/GITHUB_TOKEN").item();
        let mut asked = Vec::new();

        let restored = restore(
            &id,
            &json!("<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>"),
            None,
            &mut |key| {
                asked.push(key.clone());
                Ok(Some("entered".to_owned()))
            },
        )
        .unwrap();

        assert_eq!(restored.value, Some(json!("entered")));
        assert_eq!(restored.missing, []);
        assert_eq!(asked, [key(".claude/settings.json:/env/GITHUB_TOKEN")]);
    }

    #[test]
    fn a_secret_stays_missing_until_the_machine_has_a_value_or_the_remote_drops_it() {
        let set = key(".claude/settings.json:/env/SET_TOKEN");
        let dropped = key(".claude/settings.json:/env/DROPPED_TOKEN");
        let open = key(".claude/settings.json:/env/OPEN_TOKEN");
        let setup = settings_items(json!({"env": {"SET_TOKEN": "x"}})).unwrap();
        let remote = settings_items(json!({"env": {
            "SET_TOKEN": "<harness-sync secret .claude/settings.json:/env/SET_TOKEN>",
            "OPEN_TOKEN": "<harness-sync secret .claude/settings.json:/env/OPEN_TOKEN>"
        }}))
        .unwrap();

        let missing = still_missing([set, dropped, open.clone()], &setup, &remote);

        assert_eq!(missing, BTreeSet::from([open]));
    }
}
