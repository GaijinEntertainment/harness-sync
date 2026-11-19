//! The core of harness-sync: the item model, the catalog machinery, the three-way comparison, the secret masking, the
//! sync configuration, the commands that return reports, and the traits that a harness crate and a storage crate
//! implement. A binary installs a [`registry::Registry`] of the harnesses and storage kinds that it includes, then runs
//! the commands.

pub mod compare;
pub mod config;
pub mod declarations;
pub mod declare;
pub mod harness;
pub mod harnesses;
pub mod init;
pub mod item;
pub mod paths;
pub mod pin;
pub mod pull;
pub mod push;
pub mod registry;
pub mod report;
pub mod retired;
pub mod secret;
pub mod set_secret;
pub mod status;
pub mod storage;
pub mod target;

mod atomic;
mod executables;
mod files;
mod home;
mod json_edit;
mod references;
mod session;
mod setup;
