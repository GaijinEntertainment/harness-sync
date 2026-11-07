//! The three-way comparison of each supported item against the base commit (ADR 1.3).

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::item::{ItemId, Items};

/// How a supported item differs between the local setup and the remote snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Only the local setup changed the item since the base commit.
    LocalChange,
    /// Only the remote snapshot changed the item since the base commit.
    RemoteChange,
    /// Both sides changed the item since the base commit, to different values.
    Conflict,
    /// The item differs, and a pin keeps it as it is on this machine.
    Pinned,
    /// The item is a secret that this machine has no value for.
    SecretMissing,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Difference<'a> {
    pub(crate) id: &'a ItemId,
    pub(crate) state: ItemState,
    pub(crate) local: Option<&'a Value>,
    pub(crate) remote: Option<&'a Value>,
}

#[derive(Debug)]
pub(crate) struct ThreeWay {
    pub(crate) base: Items,
    pub(crate) local: Items,
    pub(crate) remote: Items,
}

impl ThreeWay {
    pub(crate) fn differences(&self) -> Vec<Difference<'_>> {
        let ids: BTreeSet<&ItemId> = self
            .base
            .keys()
            .chain(self.local.keys())
            .chain(self.remote.keys())
            .collect();
        ids.into_iter()
            .filter_map(|id| self.difference(id))
            .collect()
    }

    fn difference<'a>(&'a self, id: &'a ItemId) -> Option<Difference<'a>> {
        let base = self.base.get(id);
        let local = self.local.get(id);
        let remote = self.remote.get(id);
        if local == remote {
            return None;
        }
        let state = if base == local {
            ItemState::RemoteChange
        } else if base == remote {
            ItemState::LocalChange
        } else {
            ItemState::Conflict
        };
        Some(Difference {
            id,
            state,
            local,
            remote,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::item::settings_items;

    fn three_way(base: Value, local: Value, remote: Value) -> ThreeWay {
        ThreeWay {
            base: settings_items(base).unwrap(),
            local: settings_items(local).unwrap(),
            remote: settings_items(remote).unwrap(),
        }
    }

    fn states(three_way: &ThreeWay) -> Vec<(String, ItemState)> {
        three_way
            .differences()
            .iter()
            .map(|d| (d.id.to_string(), d.state))
            .collect()
    }

    #[test]
    fn equal_sides_have_no_differences() {
        let sides = three_way(json!({"a": 1}), json!({"a": 2}), json!({"a": 2}));

        assert_eq!(sides.differences(), []);
    }

    #[test]
    fn a_change_on_one_side_belongs_to_that_side() {
        let sides = three_way(
            json!({"a": 1, "b": 1}),
            json!({"a": 2, "b": 1}),
            json!({"a": 1, "b": 3}),
        );

        assert_eq!(
            states(&sides),
            [
                (
                    ".claude/settings.json:/a".to_owned(),
                    ItemState::LocalChange
                ),
                (
                    ".claude/settings.json:/b".to_owned(),
                    ItemState::RemoteChange
                )
            ]
        );
    }

    #[test]
    fn a_deletion_is_a_change() {
        let sides = three_way(json!({"a": 1, "b": 1}), json!({"b": 1}), json!({"a": 1}));

        assert_eq!(
            states(&sides),
            [
                (
                    ".claude/settings.json:/a".to_owned(),
                    ItemState::LocalChange
                ),
                (
                    ".claude/settings.json:/b".to_owned(),
                    ItemState::RemoteChange
                )
            ]
        );
    }

    #[test]
    fn different_changes_on_both_sides_conflict() {
        let sides = three_way(json!({"a": 1}), json!({"a": 2}), json!({"a": 3}));

        assert_eq!(
            states(&sides),
            [(".claude/settings.json:/a".to_owned(), ItemState::Conflict)]
        );
    }

    #[test]
    fn without_a_base_one_sided_items_come_from_their_side_and_the_rest_conflicts() {
        let sides = three_way(json!({}), json!({"a": 1, "c": 1}), json!({"b": 1, "c": 2}));

        assert_eq!(
            states(&sides),
            [
                (
                    ".claude/settings.json:/a".to_owned(),
                    ItemState::LocalChange
                ),
                (
                    ".claude/settings.json:/b".to_owned(),
                    ItemState::RemoteChange
                ),
                (".claude/settings.json:/c".to_owned(), ItemState::Conflict)
            ]
        );
    }

    #[test]
    fn key_order_inside_a_value_is_not_a_difference() {
        let sides = three_way(
            json!({}),
            json!({"p": {"x": 1, "y": 2}}),
            json!({"p": {"y": 2, "x": 1}}),
        );

        assert_eq!(sides.differences(), []);
    }
}
