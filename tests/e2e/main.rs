//! End-to-end tests: the `cc-sync` binary runs against simulated machines and a bare git repo on disk.

#[cfg(test)]
mod claude_json;
#[cfg(test)]
mod files;
#[cfg(test)]
mod init;
#[cfg(test)]
mod marketplaces;
#[cfg(test)]
mod pull;
#[cfg(test)]
mod push;
#[cfg(test)]
mod secret;
#[cfg(test)]
mod status;
#[cfg(test)]
mod support;
