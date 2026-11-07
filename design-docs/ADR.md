# Architecture decisions

Each entry is a decision in force, stated as its title; the linked record holds the rule in full, why it holds, what
lost, and what would reopen it.

- Before changing code under an area below, read the records listed for it.
- A task that conflicts with a listed decision stops and asks; it does not change the decision or work around it.
- Writing, changing, or removing a record follows the `the-blueprint:adr` skill.

## /

- [1.1 Sync only the items in the catalog, one JSON key at a time](/design-docs/adr/1.1-supported-items.md)
- [1.2 Mask secrets on push and keep each machine's own value on pull](/design-docs/adr/1.2-secret-handling.md)
- [1.3 Compare each item three ways against the base commit, and stop on conflicts](/design-docs/adr/1.3-concurrent-changes.md)
- [1.4 Sync the plugin declarations and let Claude Code install the plugins](/design-docs/adr/1.4-plugin-setup.md)
- [1.5 Handle per-machine differences with pins, a home-path token, and warnings](/design-docs/adr/1.5-per-machine-differences.md)
- [1.6 Change only synced values in Claude Code files, with a backup and an atomic replace](/design-docs/adr/1.6-target-file-writes.md)
- [1.7 Run git operations through the git executable](/design-docs/adr/1.7-git-access.md)
- [1.8 Build a CLI that an agent can operate without a TTY, and no TUI](/design-docs/adr/1.8-operator-interface.md)
- [1.9 Write cc-sync in Rust](/design-docs/adr/1.9-implementation-language.md)
- [1.10 Pull from one primary target and push to the other targets on request](/design-docs/adr/1.10-sync-targets.md)
- [1.11 Send the snapshot to a Gerrit target as a change for review, and never merge it](/design-docs/adr/1.11-gerrit-targets.md)
