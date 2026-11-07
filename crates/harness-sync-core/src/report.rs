//! The report that each command returns, which the binary prints as text or as JSON.

use serde::Serialize;
use serde_json::Value;

use crate::compare::{Difference, ItemState};
use crate::item::KeyPath;
use crate::secret;

/// How a command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The command did its work.
    Ok,
    /// Pull stopped on conflicts and wrote nothing.
    Conflicts,
    /// Push refused, because the remote snapshot has commits that this machine has not pulled.
    RemoteAhead,
    /// Push refused, because the outgoing setup holds token-like values outside secret placeholders.
    SecretsFound,
}

impl Outcome {
    /// The exit code of the process for this outcome.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Conflicts => 3,
            Self::RemoteAhead => 4,
            Self::SecretsFound => 5,
        }
    }
}

/// The result of one command.
#[derive(Debug, Serialize)]
pub struct Report {
    /// The command, such as `push` or `target list`.
    pub command: &'static str,
    /// How the command ended.
    pub outcome: Outcome,
    /// The base commit of this machine after the command.
    pub base_commit: Option<String>,
    /// The commit of the remote snapshot of the primary target after the command.
    pub remote_commit: Option<String>,
    /// The items that differ between the local setup and the remote snapshot.
    pub items: Vec<ItemReport>,
    /// The key paths of the token-like values that blocked a push.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secrets_found: Vec<String>,
    /// The commands of synced items whose executables this machine does not have.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_executables: Vec<MissingExecutable>,
    /// The marketplaces that do not sync, because the settings do not declare them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub undeclared_marketplaces: Vec<UndeclaredMarketplace>,
    /// The targets of this machine, for `target list`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TargetReport>,
    /// The harnesses that harness-sync knows, for `harness list` and `init`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub harnesses: Vec<HarnessReport>,
    /// The result of each push to a target.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pushes: Vec<TargetPush>,
    /// The lists of `harness-sync.toml` of a target, for the `declarations` commands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declarations: Option<DeclarationsReport>,
    /// One line for a person, which the JSON form leaves out.
    #[serde(skip)]
    pub summary: String,
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
            harnesses: Vec::new(),
            pushes: Vec::new(),
            declarations: None,
            summary,
        }
    }
}

/// A target of this machine.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TargetReport {
    /// The name of the target.
    pub name: String,
    /// The URL or path of the remote.
    pub url: String,
    /// The branch that holds the remote snapshot.
    pub branch: String,
    /// The storage kind of the target.
    pub kind: String,
    /// Whether the target is the primary target.
    pub primary: bool,
}

/// A harness that harness-sync knows.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct HarnessReport {
    /// The name of the harness.
    pub name: &'static str,
    /// Whether this machine syncs the harness.
    pub synced: bool,
}

/// The result of a push to one target.
#[derive(Debug, Serialize)]
pub struct TargetPush {
    /// The name of the target.
    pub target: String,
    /// How the push to the target ended.
    pub outcome: Outcome,
    /// The commit of the target after the push.
    pub commit: Option<String>,
    /// The change that waits for review, when the target takes changes for review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<Review>,
    /// The items that the push replaced on a target other than the primary.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ItemReport>,
}

/// The lists of `harness-sync.toml` of a target.
#[derive(Debug, Serialize)]
pub struct DeclarationsReport {
    /// The name of the target.
    pub target: String,
    /// The `secrets` list.
    pub secrets: Vec<String>,
    /// The `include` list.
    pub include: Vec<String>,
    /// The `exclude` list.
    pub exclude: Vec<String>,
}

/// A commit sent for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Review {
    /// The commit that waits for review.
    pub commit: String,
    /// The URL of the change, when the target reported one.
    pub change_url: Option<String>,
}

/// A command of a synced item whose executable this machine does not have.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MissingExecutable {
    /// The key path of the command.
    pub id: String,
    /// The executable that the command runs.
    pub executable: String,
}

/// A marketplace that does not sync, because the settings do not declare it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UndeclaredMarketplace {
    /// The name of the marketplace.
    pub name: String,
    /// The declaration to add to the settings, with secrets masked.
    pub declaration: Value,
}

/// A supported item that differs between the local setup and the remote snapshot.
#[derive(Debug, Serialize)]
pub struct ItemReport {
    /// The item id.
    pub id: String,
    /// How the item differs.
    pub state: ItemState,
    /// The local value, with secrets masked.
    pub local: Option<Value>,
    /// The remote value, with secrets masked.
    pub remote: Option<Value>,
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
