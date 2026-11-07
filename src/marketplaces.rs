use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::declarations::Declarations;
use crate::item::{CLAUDE_SETTINGS, ItemId, Items};
use crate::paths::Paths;
use crate::report::UndeclaredMarketplace;
use crate::secret;

const DECLARATION_KEY: &str = "extraKnownMarketplaces";
const DECLARATION_FIELDS: [&str; 2] = ["source", "autoUpdate"];

pub(crate) fn undeclared(
    paths: &Paths,
    setup: &Items,
    declarations: &Declarations,
) -> Result<Vec<UndeclaredMarketplace>> {
    let path = paths.known_marketplaces_file();
    let known: Map<String, Value> = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("cannot read the marketplaces of {}", path.display()))?,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    Ok(known
        .into_iter()
        .filter_map(|(name, record)| {
            let id = ItemId::key(CLAUDE_SETTINGS, &[DECLARATION_KEY, &name]);
            if setup.contains_key(&id) || declarations.excludes(&id) {
                return None;
            }
            let declaration: Map<String, Value> = record
                .as_object()?
                .iter()
                .filter(|(field, _)| DECLARATION_FIELDS.contains(&field.as_str()))
                .map(|(field, value)| (field.clone(), value.clone()))
                .collect();
            Some(UndeclaredMarketplace {
                declaration: secret::redact(&id, &Value::Object(declaration)),
                name,
            })
        })
        .collect())
}
