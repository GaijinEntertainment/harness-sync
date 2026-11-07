use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::slice;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

use crate::git::{Git, TreeChange};
use crate::home;
use crate::item::{ItemId, Items, JsonFile, Source};
use crate::paths::Paths;

const SNAPSHOT_FILE: &str = "plugins/marketplaces.json";
const AUTO_UPDATE: &str = "autoUpdate";
const SYNCED_FIELDS: [&str; 2] = ["source", AUTO_UPDATE];
const DECLARATION_KEY: &str = "extraKnownMarketplaces";

pub(crate) fn load_local(paths: &Paths, settings: &Items) -> Result<Items> {
    let path = paths.known_marketplaces_file();
    let mut items = match fs::read(&path) {
        Ok(bytes) => parse(&bytes)
            .with_context(|| format!("cannot read the marketplaces of {}", path.display()))?,
        Err(err) if err.kind() == ErrorKind::NotFound => Items::new(),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    for (name, declaration) in declarations(settings) {
        if let Some(fields) = items
            .entry(item_id(name))
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
        {
            fields.extend(synced_fields(declaration));
        }
    }
    Ok(items)
}

pub(crate) fn declared_names(settings: &Items) -> BTreeSet<String> {
    declarations(settings)
        .map(|(name, _)| name.to_owned())
        .collect()
}

fn declarations(settings: &Items) -> impl Iterator<Item = (&str, &Value)> {
    settings
        .iter()
        .filter_map(|(id, value)| match (id.source, id.path.as_slice()) {
            (Source::Json(JsonFile::Settings), [key, name]) if key == DECLARATION_KEY => {
                Some((name.as_str(), value))
            }
            _ => None,
        })
}

pub(crate) fn load_snapshot(git: &Git, commit: &str) -> Result<Items> {
    let spec = format!("{commit}:{SNAPSHOT_FILE}");
    git.read_files(slice::from_ref(&spec))?
        .into_iter()
        .next()
        .flatten()
        .map_or_else(
            || Ok(Items::new()),
            |bytes| {
                parse(&bytes).with_context(|| format!("cannot read the marketplaces of {spec}"))
            },
        )
}

fn parse(bytes: &[u8]) -> Result<Items> {
    let Value::Object(marketplaces) = serde_json::from_slice(bytes)? else {
        bail!("the document does not hold a JSON object");
    };
    Ok(marketplaces
        .into_iter()
        .map(|(name, record)| (item_id(&name), Value::Object(synced_fields(&record))))
        .collect())
}

fn synced_fields(record: &Value) -> Map<String, Value> {
    record
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(field, _)| SYNCED_FIELDS.contains(&field.as_str()))
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect()
}

pub(crate) fn snapshot_change(items: &Items) -> Result<TreeChange> {
    let document: Map<String, Value> = items
        .iter()
        .filter(|(id, _)| id.source == Source::Marketplace)
        .map(|(id, value)| (id.path.join("/"), value.clone()))
        .collect();
    let mut content =
        serde_json::to_vec_pretty(&document).context("cannot serialize the marketplaces")?;
    content.push(b'\n');
    Ok(TreeChange::Write {
        path: SNAPSHOT_FILE.to_owned(),
        content,
        executable: false,
    })
}

pub(crate) fn apply(
    paths: &Paths,
    changes: &[(&ItemId, Option<&Value>)],
    declared: &BTreeSet<String>,
) -> Vec<String> {
    changes
        .iter()
        .filter(|(id, _)| !declared.contains(&id.path.join("/")))
        .filter_map(|(id, value)| {
            arguments(paths, id, *value)
                .and_then(|args| run_claude(&args))
                .err()
                .map(|err| format!("{id}: {err:#}"))
        })
        .collect()
}

pub(crate) fn auto_updates<'a>(
    changes: &[(&ItemId, Option<&'a Value>)],
    declared: &BTreeSet<String>,
) -> Vec<(Vec<String>, &'a Value)> {
    changes
        .iter()
        .filter(|(id, _)| !declared.contains(&id.path.join("/")))
        .filter_map(|(id, value)| {
            let flag = (*value)?.get(AUTO_UPDATE)?;
            let path = [DECLARATION_KEY, &id.path.join("/"), AUTO_UPDATE].map(str::to_owned);
            Some((path.to_vec(), flag))
        })
        .collect()
}

fn arguments(paths: &Paths, id: &ItemId, value: Option<&Value>) -> Result<Vec<String>> {
    let mut args: Vec<String> = ["plugin", "marketplace"].map(str::to_owned).to_vec();
    let Some(value) = value else {
        args.extend(["remove".to_owned(), id.path.join("/")]);
        return Ok(args);
    };
    let mut value = value.clone();
    home::expand(&mut value, &paths.home);
    let source = source_argument(&value).context(
        "the source of the marketplace has no form that `claude plugin marketplace add` takes",
    )?;
    args.extend(["add".to_owned(), source]);
    Ok(args)
}

fn source_argument(value: &Value) -> Option<String> {
    let source = value.get("source")?;
    let text = |field: &str| source.get(field).and_then(Value::as_str);
    let base = match text("source")? {
        "github" => text("repo")?,
        "git" | "url" => text("url")?,
        "directory" | "file" => text("path")?,
        _ => return None,
    };
    Some(text("ref").map_or_else(
        || base.to_owned(),
        |reference| format!("{base}#{reference}"),
    ))
}

fn run_claude(args: &[String]) -> Result<()> {
    let output = Command::new("claude")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow::anyhow!("the `claude` command is not on PATH")
            } else {
                anyhow::Error::new(err).context("cannot run `claude`")
            }
        })?;
    if !output.status.success() {
        bail!(
            "`claude {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn item_id(name: &str) -> ItemId {
    ItemId {
        source: Source::Marketplace,
        path: vec![name.to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::item::settings_items;

    #[test]
    fn a_declaration_sets_the_auto_update_of_its_marketplace_over_the_install_record() {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::for_home(home.path());
        let known = paths.known_marketplaces_file();
        fs::create_dir_all(known.parent().unwrap()).unwrap();
        fs::write(
            &known,
            br#"{"acme": {"source": {"source": "github", "repo": "acme/plugins"}, "autoUpdate": true},
                 "team": {"source": {"source": "github", "repo": "team/plugins"}, "autoUpdate": true}}"#,
        )
        .unwrap();
        let settings = settings_items(json!({"extraKnownMarketplaces": {
            "acme": {"source": {"source": "github", "repo": "acme/plugins"}, "autoUpdate": false},
            "team": {"source": {"source": "github", "repo": "team/plugins"}}
        }}))
        .unwrap();

        let items = load_local(&paths, &settings).unwrap();

        assert_eq!(items[&item_id("acme")]["autoUpdate"], false);
        assert_eq!(items[&item_id("team")]["autoUpdate"], true);
    }

    #[test]
    fn a_marketplace_item_keeps_the_source_and_drops_the_install_record() {
        let items = parse(
            br#"{"acme": {"source": {"source": "github", "repo": "acme/plugins"},
                 "installLocation": "/Users/a/.claude/plugins/marketplaces/acme",
                 "lastUpdated": "2026-11-01T00:00:00Z", "autoUpdate": true}}"#,
        )
        .unwrap();

        assert_eq!(
            items[&item_id("acme")],
            json!({"source": {"source": "github", "repo": "acme/plugins"}, "autoUpdate": true})
        );
    }

    #[test]
    fn each_source_form_gives_the_argument_of_marketplace_add() {
        let argument = |source: Value| source_argument(&json!({"source": source}));

        assert_eq!(
            argument(json!({"source": "github", "repo": "o/r"})).as_deref(),
            Some("o/r")
        );
        assert_eq!(
            argument(json!({"source": "github", "repo": "o/r", "ref": "v1"})).as_deref(),
            Some("o/r#v1")
        );
        assert_eq!(
            argument(json!({"source": "git", "url": "https://h/x.git"})).as_deref(),
            Some("https://h/x.git")
        );
        assert_eq!(
            argument(json!({"source": "directory", "path": "/m"})).as_deref(),
            Some("/m")
        );
        assert_eq!(argument(json!({"source": "npm", "package": "p"})), None);
    }
}
