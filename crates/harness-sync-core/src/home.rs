use std::path::Path;

use serde_json::Value;

const HOME_TOKEN: &str = "<harness-sync home>";

pub(crate) fn tokenize(value: &mut Value, home: &Path) {
    let text = home.to_string_lossy();
    let native = text.trim_end_matches(['/', '\\']);
    if native.len() < 2 {
        return;
    }
    let slashed = native.replace('\\', "/");
    for_each_string(value, &mut |string| {
        let mut tokenized = replace_home(string, native);
        if slashed != native {
            tokenized = replace_home(&tokenized, &slashed);
        }
        *string = tokenized;
    });
}

pub(crate) fn expand(value: &mut Value, home: &Path) {
    let text = home.to_string_lossy();
    let home = if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    };
    for_each_string(value, &mut |string| {
        if string.contains(HOME_TOKEN) {
            *string = string.replace(HOME_TOKEN, &home);
        }
    });
}

fn replace_home(text: &str, home: &str) -> String {
    let is_path_char = |c: char| c.is_alphanumeric() || matches!(c, '.' | '_' | '-');
    let mut replaced = String::with_capacity(text.len());
    let mut copied = 0;
    for (start, _) in text.match_indices(home) {
        let end = start + home.len();
        let starts_path = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_path_char(c) && !matches!(c, '/' | '\\'));
        let ends_home = text[end..].chars().next().is_none_or(|c| !is_path_char(c));
        if start >= copied && starts_path && ends_home {
            replaced.push_str(&text[copied..start]);
            replaced.push_str(HOME_TOKEN);
            copied = end;
        }
    }
    replaced.push_str(&text[copied..]);
    replaced
}

fn for_each_string(value: &mut Value, f: &mut impl FnMut(&mut String)) {
    match value {
        Value::String(text) => f(text),
        Value::Object(entries) => entries
            .values_mut()
            .for_each(|child| for_each_string(child, f)),
        Value::Array(elements) => elements
            .iter_mut()
            .for_each(|child| for_each_string(child, f)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tokenized(value: Value, home: &str) -> Value {
        let mut value = value;
        tokenize(&mut value, Path::new(home));
        value
    }

    #[test]
    fn the_home_prefix_of_a_path_becomes_the_token() {
        let value = json!({
            "command": "node /Users/a/.claude/statusline.js --short",
            "list": ["/Users/a", "/Users/a/x:/Users/a/y"]
        });

        assert_eq!(
            tokenized(value, "/Users/a"),
            json!({
                "command": "node <harness-sync home>/.claude/statusline.js --short",
                "list": ["<harness-sync home>", "<harness-sync home>/x:<harness-sync home>/y"]
            })
        );
    }

    #[test]
    fn a_longer_name_or_a_deeper_path_keeps_its_text() {
        let value = json!(["/Users/ab/x", "/mnt/Users/a/x", "/Users/a.old/x", "$HOME/x"]);

        assert_eq!(tokenized(value.clone(), "/Users/a"), value);
    }

    #[test]
    fn a_windows_home_matches_with_either_separator() {
        let value = json!(["C:\\Users\\a\\.claude\\x.ps1", "C:/Users/a/.claude/x.sh"]);

        assert_eq!(
            tokenized(value, "C:\\Users\\a"),
            json!([
                "<harness-sync home>\\.claude\\x.ps1",
                "<harness-sync home>/.claude/x.sh"
            ])
        );
    }

    #[test]
    fn expand_puts_the_home_of_the_target_in_place_of_the_token() {
        let mut value = json!({"command": "bash <harness-sync home>/.claude/x.sh"});

        expand(&mut value, Path::new("/home/b"));

        assert_eq!(value, json!({"command": "bash /home/b/.claude/x.sh"}));
    }
}
