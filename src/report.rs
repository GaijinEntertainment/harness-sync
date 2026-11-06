use serde::Serialize;
use serde_json::Value;

use crate::compare::{Difference, ItemState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Ok,
}

impl Outcome {
    pub(crate) const fn exit_code(self) -> u8 {
        match self {
            Self::Ok => 0,
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
    #[serde(skip)]
    pub(crate) summary: String,
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
            local: difference.local.cloned(),
            remote: difference.remote.cloned(),
        }
    }
}
