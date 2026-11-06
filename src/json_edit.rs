use anyhow::{Context, Result};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use serde_json::Value;

#[derive(Debug)]
pub(crate) struct Edit<'a> {
    pub(crate) path: &'a [String],
    pub(crate) value: Option<&'a Value>,
}

pub(crate) fn apply(text: &str, edits: &[Edit<'_>]) -> Result<String> {
    let root = CstRootNode::parse(text, &ParseOptions::default())?;
    let object = root
        .object_value()
        .context("the document is not a JSON object")?;
    for edit in edits {
        match edit.value {
            Some(value) => set(&object, edit.path, value),
            None => remove(&object, edit.path),
        }
    }
    Ok(root.to_string())
}

fn set(object: &CstObject, path: &[String], value: &Value) {
    match path {
        [] => {}
        [key] => match object.get(key) {
            Some(property) => property.set_value(input(value)),
            None => drop(object.append(key, input(value))),
        },
        [key, rest @ ..] => set(&object.object_value_or_set(key), rest, value),
    }
}

fn remove(object: &CstObject, path: &[String]) {
    match path {
        [] => {}
        [key] => {
            if let Some(property) = object.get(key) {
                property.remove();
            }
        }
        [key, rest @ ..] => {
            if let Some(child) = object.object_value(key) {
                remove(&child, rest);
            }
        }
    }
}

fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(flag) => CstInputValue::Bool(*flag),
        Value::Number(number) => CstInputValue::Number(number.to_string()),
        Value::String(text) => CstInputValue::String(text.clone()),
        Value::Array(values) => CstInputValue::Array(values.iter().map(input).collect()),
        Value::Object(entries) => CstInputValue::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), input(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn path(segments: &[&str]) -> Vec<String> {
        segments.iter().map(|&segment| segment.to_owned()).collect()
    }

    #[test]
    fn an_edit_keeps_the_bytes_outside_the_edited_value() {
        let text = "{\n\t\"keep\":   [1,2],\n\t\"model\": \"opus\",\n\t\"tail\": true\n}\n";
        let model = path(&["model"]);
        let value = json!("sonnet");

        let edited = apply(
            text,
            &[Edit {
                path: &model,
                value: Some(&value),
            }],
        )
        .unwrap();

        assert_eq!(
            edited,
            "{\n\t\"keep\":   [1,2],\n\t\"model\": \"sonnet\",\n\t\"tail\": true\n}\n"
        );
    }

    #[test]
    fn a_nested_set_creates_the_parent_object() {
        let text = "{\n  \"a\": 1\n}";
        let env = path(&["env", "NAME"]);
        let value = json!("1");

        let edited = apply(
            text,
            &[Edit {
                path: &env,
                value: Some(&value),
            }],
        )
        .unwrap();

        let parsed: Value = serde_json::from_str(&edited).unwrap();
        assert_eq!(parsed, json!({"a": 1, "env": {"NAME": "1"}}));
        assert!(edited.starts_with("{\n  \"a\": 1,\n"), "edited: {edited}");
    }

    #[test]
    fn a_removal_deletes_only_the_named_entry() {
        let text = "{\n  \"env\": {\n    \"A\": \"1\",\n    \"B\": \"2\"\n  }\n}\n";
        let a = path(&["env", "A"]);
        let missing = path(&["nothing", "here"]);

        let edited = apply(
            text,
            &[
                Edit {
                    path: &a,
                    value: None,
                },
                Edit {
                    path: &missing,
                    value: None,
                },
            ],
        )
        .unwrap();

        assert_eq!(edited, "{\n  \"env\": {\n    \"B\": \"2\"\n  }\n}\n");
    }

    #[test]
    fn numbers_keep_their_text() {
        let value: Value = serde_json::from_str("12345678901234567890.50").unwrap();
        let n = path(&["n"]);

        let edited = apply(
            "{}",
            &[Edit {
                path: &n,
                value: Some(&value),
            }],
        )
        .unwrap();

        assert!(
            edited.contains("12345678901234567890.50"),
            "edited: {edited}"
        );
    }

    #[test]
    fn a_document_that_is_not_an_object_is_rejected() {
        assert!(apply("[1]", &[]).is_err());
        assert!(apply("{ broken", &[]).is_err());
    }
}
