use std::process;
use std::time::SystemTime;

use anyhow::Result;

use crate::config::Kind;
use crate::git::Git;

pub(crate) fn destination(kind: Kind, branch: &str) -> String {
    match kind {
        Kind::Git => format!("refs/heads/{branch}"),
        Kind::Gerrit => format!("refs/for/{branch}"),
    }
}

pub(crate) fn message(
    git: &Git,
    kind: Kind,
    subject: String,
    parent: Option<&str>,
) -> Result<String> {
    match kind {
        Kind::Git => Ok(subject),
        Kind::Gerrit => {
            let seed = format!(
                "{subject}\n{}\n{:?}\n{}",
                parent.unwrap_or_default(),
                SystemTime::now(),
                process::id()
            );
            Ok(format!(
                "{subject}\n\nChange-Id: I{}",
                git.hash_text(&seed)?
            ))
        }
    }
}

pub(crate) fn change_url(remote_messages: &str) -> Option<String> {
    remote_messages
        .lines()
        .filter_map(|line| line.strip_prefix("remote:"))
        .flat_map(str::split_whitespace)
        .find(|word| word.starts_with("https://") || word.starts_with("http://"))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_change_url_comes_from_the_remote_messages_of_gerrit() {
        let messages = "remote: \nremote: Processing changes: refs: 1, new: 1, done\nremote: New Changes:        \n\
                        remote:   https://gerrit.example/c/setup/+/42 harness-sync push from host [NEW]        \n";

        assert_eq!(
            change_url(messages).as_deref(),
            Some("https://gerrit.example/c/setup/+/42")
        );
        assert_eq!(change_url("remote: Resolving deltas\n"), None);
    }
}
