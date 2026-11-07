#[cfg(test)]
mod fixture;

use std::sync::OnceLock;

use anyhow::{Result, anyhow};

use crate::harness::Harness;
use crate::storage::StorageKind;

/// The harnesses and storage kinds that a binary includes.
#[derive(Debug)]
pub(crate) struct Registry {
    pub(crate) harnesses: &'static [&'static Harness],
    pub(crate) storage_kinds: &'static [&'static StorageKind],
}

static INSTALLED: OnceLock<&'static Registry> = OnceLock::new();

/// Makes `registry` the one that every command reads. The binary installs its registry once, before it runs a
/// command.
pub(crate) fn install(registry: &'static Registry) -> Result<()> {
    INSTALLED
        .set(registry)
        .map_err(|_| anyhow!("a registry of harnesses and storage kinds is installed already"))
}

#[cfg(not(test))]
#[expect(
    clippy::expect_used,
    reason = "a command that runs before the binary installs its registry is a bug in the binary"
)]
pub(crate) fn installed() -> &'static Registry {
    INSTALLED
        .get()
        .expect("the binary installs its registry before it runs a command")
}

#[cfg(test)]
pub(crate) fn installed() -> &'static Registry {
    INSTALLED.get_or_init(|| &fixture::REGISTRY)
}
