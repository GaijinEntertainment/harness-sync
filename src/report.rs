use serde::Serialize;

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
    pub(crate) remote_commit: Option<String>,
    #[serde(skip)]
    pub(crate) summary: String,
}
