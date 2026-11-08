# Alignment: harness-sync

- **Date:** 2026-11-07
- **Brief:** `design-docs/01-harness-sync.brief.md`
- **Research:** `design-docs/01-harness-sync.research.md`
- **Glossary:** `docs/glossary.md`

The user delegated the decisions of this alignment ("there are no significant decisions for me to take from here, which
would not be covered by decisions of initial DRAFT"). Each decision below follows the brief, the research, and the
decisions the user made for the Claude Code design; the ADRs record them.

## Patterns Adopted

- **Catalog of supported items, per harness** — only the items in a harness's catalog sync. Found in most Claude Code
  sync tools of the research, and in `hjnnjh/coding-agent-config-sync` (items listed per tool). Every harness
  directory mixes the setup with state that the harness writes.
- **Key-level JSON items** — one item per top-level key, and one per entry of a map of independent entries. Found in
  `Hudint/claudesync`, `zhaojiannet/cclayer`, `CharlesGuooo/ai-agent-config-sync` ("merge by entry"). Secrets and home
  paths sit next to portable keys in Claude Code and pi files alike.
- **No conversion between harnesses** — each harness keeps its own files. The tools that convert (`spxrogers/agentsync`,
  `rulesync`, `ruler`, `vsync`) report lossy cases; the brief makes interop a non-goal.
- **Repo as a mirror of the home directory** — the colleague's hand-maintained repo and `rosinbum/agent-config-sync`
  keep one folder per harness named after its directory; yadm tracks files in place under `$HOME`. The item id is the
  home-relative path, with a JSON pointer for a key.
- **Secret placeholders, name rules, and a scan gate** — as in the Claude Code design; `legout/pi-config` uses
  `__REQUIRED__` placeholders for pi. The name rule grows to match `apiKey` and `api-key` (research: pi and kensai key
  names that the old rule missed).
- **Home-path token, pins, three-way comparison, in-place patch** — unchanged from the Claude Code design; every
  harness gets them through the core.
- **Plugin declarations, installed by the harness** — Claude Code clones declared marketplaces at startup; pi installs
  declared packages (`docs/packages.md`). harness-sync syncs declarations and runs no harness command.
- **Compile-time plugins** — harness crates and storage crates are workspace crates linked into one binary. No stable
  Rust ABI exists for run-time loading, and an out-of-process protocol is a second product.
- **Storage behind a trait, git as the first implementation** — the existing code reaches git through one module
  (`src/git.rs`); Gerrit differs only in the commit message, the destination ref, the change URL, and whether the base
  moves.
- **The git executable** — unchanged.

## Current State

harness-sync is one binary crate that syncs Claude Code only. Claude Code names sit in `item.rs` (`JsonFile`, the key
lists), `paths.rs`, `files.rs`, `references.rs`, `executables.rs`, and `marketplaces.rs`; the comparison, git, Gerrit,
targets, atomic writes, and the home token name no harness. Item ids are `settings:/…`, `claude:/…`, and `file:…`; the
remote snapshot holds `settings.json`, `claude.json`, and `files/` at its root. A machine syncs everything in the
catalog; it has no list of harnesses. One machine is connected, and nothing is released.

## Desired End State

- **Harnesses.** A harness crate describes one harness: its name, its directories (with the environment variable that
  moves them), its catalog (keyed JSON files with their key rules, whole files, catalog directories, paths that never
  sync), the commands that its items name, and its own warnings. The first release has `claude-code` and `pi`.
- **Machines.** The sync configuration lists the harnesses of the machine. `status`, `push`, and `pull` touch only those
  harnesses and the root files; the folders of other harnesses in the repo stay as they are.
- **Repo layout and ids.** The repo mirrors the home directory: `.claude/settings.json`, `.claude.json`,
  `.claude/rules/rust.md`, `.pi/agent/settings.json`. A keyed JSON file holds only its synced keys. An item id is the
  home-relative path, plus `:` and a JSON pointer for a key: `.claude/settings.json:/env/EDITOR`. A path that no
  harness owns is a root file, and only the repo's `include` list adds root files.
- **Repo settings.** One `harness-sync.toml` at the repo root holds `secrets`, `include`, and `exclude` as item ids.
- **Storage.** The core defines a storage trait: fetch a target, read a snapshot, commit changes on a parent, publish
  with an outcome (published, sent for review, or refused because the remote is ahead), and content ids for files. The
  git crate implements it for the `git` and `gerrit` kinds through the `git` executable.
- **Crates.** `harness-sync-core` (item model, catalog machinery, comparison, secrets, home token, JSON edits, writes,
  repo settings, sync configuration, commands that return reports, the harness and storage traits),
  `harness-sync-claude-code`, `harness-sync-pi`, `harness-sync-git`, and the `harness-sync` binary (CLI, terminal
  output, prompts, and the list of compiled-in harnesses and storage kinds).
- **pi catalog.** Keyed: `settings.json` (every key except `lastChangelogVersion`, `trackingId`, `deviceId`),
  `models.json` (one item per provider), `mcp.json` (one item per server). Files: `keybindings.json`, the context and
  system prompt files, `extensions/`, `skills/`, `prompts/`, `themes/`. Never: `auth.json`, `mcp-auth.json`,
  `models-store.json`, `trust.json`, `sessions/`, `npm/`, `bin/`, logs, caches.
- **Secrets.** A key matches the name rule when, in any case, it is `Authorization`, or ends with `_KEY`, `-KEY`,
  `APIKEY`, `TOKEN`, `SECRET`, `PASSWORD`, or `HEADERS`.

## Resolved Questions

- Which harnesses? → Claude Code and pi in the first release; Codex and kensai-local later (the user).
- Interop between harnesses? → None; a root file is a plain file (the user).
- Storage kinds? → git and Gerrit, as two storage kinds of one git crate, behind a trait for later kinds (the user;
  Gerrit is too small for its own crate).
- Run-time plugins? → No; everything compiles into one binary (the user).
- A machine without a harness? → Its sync configuration lists its harnesses (the user).
- Form of an item id → the home-relative path, plus `:` and a JSON pointer. A harness-prefixed form
  (`claude-code:settings:/model`) needs a source name per file and a second mapping from ids to repo paths.
- Separator before the JSON pointer → `:`. `#` is a glob operator in zsh with `extendedglob`.
- One `harness-sync.toml` or one per harness → one at the root; ids carry the harness through the path.
- Where `~/.claude.json` goes → `.claude.json` at the repo root, owned by the Claude Code harness, as at the home root.
- `~/.agents/skills`, which pi and Codex share → root files; the `include` list adds them.
- Content ids of files → defined by the storage (git blob ids for git); the core compares them as opaque strings.
- `node_modules` → never synced, in any harness.
- Formats → JSON only in the first release; TOML and YAML editors come with Codex and kensai-local.
- History of the design docs → one design; git keeps the Claude Code-only wording.

## Not Yet Specified

- Whether the core crate is published with a stable API for crates outside this repo.
- How pi's local-path packages (a directory in `~/.pi/agent/packages/`) travel: the `include` list covers them, with
  `node_modules` skipped.
- How the list of undocumented Claude Code `~/.claude.json` keys stays current.
- How the user marks a false positive of the secret scan as not secret.

## Recorded ADRs

- [1.1 Sync each harness on its own, through its catalog of supported items](/design-docs/adr/1.1-supported-items.md)
- [1.2 Mask secrets on push and keep each machine's own value on pull](/design-docs/adr/1.2-secret-handling.md)
- [1.3 Compare each item three ways against the base commit, and stop on conflicts](/design-docs/adr/1.3-concurrent-changes.md)
- [1.4 Sync the plugin declarations and let each harness install the plugins](/design-docs/adr/1.4-plugin-setup.md)
- [1.5 Handle per-machine differences with harness lists, pins, a home-path token, and warnings](/design-docs/adr/1.5-per-machine-differences.md)
- [1.6 Change only synced values in harness files, with a backup and an atomic replace](/design-docs/adr/1.6-target-file-writes.md)
- [1.7 Run git operations through the git executable](/design-docs/adr/1.7-git-access.md)
- [1.8 Build a CLI that an agent can operate without a TTY, and no TUI](/design-docs/adr/1.8-operator-interface.md)
- [1.9 Write harness-sync in Rust](/design-docs/adr/1.9-implementation-language.md)
- [1.10 Pull from one primary target and push to the other targets on request](/design-docs/adr/1.10-sync-targets.md)
- [1.11 Send the snapshot to a Gerrit target as a change for review, and never merge it](/design-docs/adr/1.11-gerrit-targets.md)
- [1.12 Sync the Claude Code catalog](/design-docs/adr/1.12-claude-code-catalog.md)
- [1.13 Sync the pi catalog](/design-docs/adr/1.13-pi-catalog.md)
- [1.14 Mirror the home directory in the repo, and name each item by its path](/design-docs/adr/1.14-repo-layout.md)
- [1.15 Build a core crate, harness crates, and storage crates into one binary](/design-docs/adr/1.15-crate-structure.md)
- [1.16 Reach the remote snapshot through a storage trait](/design-docs/adr/1.16-storage.md)
