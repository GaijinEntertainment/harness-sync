use serde::Serialize;
use serde_json::Value;

use crate::compare::{Difference, ItemState};
use crate::config::Kind;
use crate::item::KeyPath;
use crate::secret;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Ok,
    Conflicts,
    RemoteAhead,
    SecretsFound,
}

impl Outcome {
    pub(crate) const fn exit_code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Conflicts => 3,
            Self::RemoteAhead => 4,
            Self::SecretsFound => 5,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub(crate) command: &'static str,
    pub(crate) outcome: Outcome,
    pub(crate) base_commit: Option<String>,
    pub(crate) remote_commit: Option<String>,
    pub(crate) items: Vec<ItemReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) secrets_found: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_executables: Vec<MissingExecutable>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) undeclared_marketplaces: Vec<UndeclaredMarketplace>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) targets: Vec<TargetReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) pushes: Vec<TargetPush>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) declarations: Option<DeclarationsReport>,
    #[serde(skip)]
    pub(crate) summary: String,
}

impl Report {
    pub(crate) const fn new(command: &'static str, summary: String) -> Self {
        Self {
            command,
            outcome: Outcome::Ok,
            base_commit: None,
            remote_commit: None,
            items: Vec::new(),
            secrets_found: Vec::new(),
            missing_executables: Vec::new(),
            undeclared_marketplaces: Vec::new(),
            targets: Vec::new(),
            pushes: Vec::new(),
            declarations: None,
            summary,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct TargetReport {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) branch: String,
    pub(crate) kind: Kind,
    pub(crate) primary: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct TargetPush {
    pub(crate) target: String,
    pub(crate) outcome: Outcome,
    pub(crate) commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) review: Option<Review>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) items: Vec<ItemReport>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeclarationsReport {
    pub(crate) target: String,
    pub(crate) secrets: Vec<String>,
    pub(crate) include: Vec<String>,
    pub(crate) exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Review {
    pub(crate) commit: String,
    pub(crate) change_url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct MissingExecutable {
    pub(crate) id: String,
    pub(crate) executable: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UndeclaredMarketplace {
    pub(crate) name: String,
    pub(crate) declaration: Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct ItemReport {
    pub(crate) id: String,
    pub(crate) state: ItemState,
    pub(crate) local: Option<Value>,
    pub(crate) remote: Option<Value>,
}

impl From<&Difference<'_>> for ItemReport {
    fn from(difference: &Difference<'_>) -> Self {
        Self {
            id: difference.id.to_string(),
            state: difference.state,
            local: difference
                .local
                .map(|value| secret::redact(difference.id, value)),
            remote: difference
                .remote
                .map(|value| secret::redact(difference.id, value)),
        }
    }
}

impl ItemReport {
    pub(crate) fn secret_missing(key: &KeyPath) -> Self {
        Self {
            id: key.to_string(),
            state: ItemState::SecretMissing,
            local: None,
            remote: Some(Value::String(secret::placeholder(key))),
        }
    }
}

pub(crate) fn with_missing_secrets(summary: String, missing: usize) -> String {
    if missing == 0 {
        summary
    } else {
        format!(
            "{summary}; {missing} secrets missing: set each with `harness-sync secret set <key path>`"
        )
    }
}

pub(crate) fn short_commit(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}
