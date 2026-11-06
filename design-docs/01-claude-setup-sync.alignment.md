# Alignment: Claude Code user-level setup sync

- **Date:** 2026-11-06
- **Brief:** `design-docs/01-claude-setup-sync.brief.md`
- **Research:** `design-docs/01-claude-setup-sync.research.md`
- **Glossary:** `docs/glossary.md` (created during this alignment)

## Patterns Adopted

cc-sync has no code yet. The patterns come from Claude Code's own configuration model and from the existing sync tools
in the research.

- **Catalog of supported items** — only the items in the catalog sync. Found in Hudint/claudesync, zhaojiannet/cclayer,
  aoprisan/ccsync, FelixIsaac/claude-code-sync, GiuseppeBellamacina/claude-code-sync; renefichtmueller/claude-sync and
  perfectra1n/claude-code-sync use a denylist instead. `~/.claude/` mixes the setup with state that Claude Code writes.
- **Key-level JSON items** — one item per key, and one per entry of a map of independent entries. Found in
  Hudint/claudesync (key-by-key layers), zhaojiannet/cclayer (`settings_keys`), aoprisan/ccsync (deep merge). Secrets
  and home paths sit next to portable keys.
- **`~/.claude.json` key groups** — `mcpServers`, the Global config keys, the settings keys with a `~/.claude.json`
  fallback, and six undocumented preference keys. Extends the `mcpServers`-only pattern of tawanorg/claude-sync and
  aoprisan/ccsync, because `/config` still writes preferences to `~/.claude.json`.
- **Referenced files, with `include` and `exclude` in the repo** — Hudint/claudesync has `include`; zhaojiannet/cclayer
  syncs the status line and hooks. User instructions and settings point to files that must travel with them.
- **Plugin declarations, installed by Claude Code** — found in the Claude Code plugin docs (startup install),
  Hudint/claudesync (settings keys), zhaojiannet/cclayer and LandB/claudesync (`claude plugin` CLI).
- **Secret placeholders, with a value per machine** — found in GCShao/cc-sync, shawonashraf/claude-code-sync,
  GiuseppeBellamacina/claude-code-sync, sgkarra/claude-sync.
- **Name rules and a scan that blocks push** — found in Hudint/claudesync, aoprisan/ccsync, jamie950315/ccsync.
- **Home-path token** — found in Hudint/claudesync `{{home}}`, perfectra1n/claude-code-sync `__HOME__`,
  tawanorg/claude-sync `${HOME}`.
- **Machine-local pins, no layers** — partially follows Hudint/claudesync, which has both `pin` and OS and host layers.
  cc-sync takes the pins only.
- **Three-way comparison per item** — found in perfectra1n/claude-code-sync, Cha1000000/claude-code-sync,
  renefichtmueller/claude-sync. cc-sync gives no side a default win.
- **In-place patch, backup, atomic replace** — aoprisan/ccsync backs up before a restore; Claude Code keeps its own
  backups of `~/.claude.json`.
- **The git executable** — no prior-art evidence; chosen because the repo can be on any git remote.

## Current State

The user-level setup is spread over `~/.claude/settings.json`, `mcpServers` and about 20 preference keys among about 90
state keys in `~/.claude.json`, authored files under `~/.claude/`, and plugin install records. Claude Code writes its
own history, caches, sessions, and identity into the same places. Claude Code has no native sync of the user-level
setup; the account sync covers only claude.ai plugins, skills, and connectors. The user moves the setup by hand when a
machine is set up and after each change. About 40 sync tools exist; none combines git storage, a three-way comparison
without a master, secret placeholders with entry on the target machine, and plugin and MCP sync.

## Desired End State

- **Commands.** `cc-sync init <remote>` connects a machine to the repo. `cc-sync status` lists the supported items that
  differ from the remote snapshot. `cc-sync push` and `cc-sync pull` show the same list before they act. Sync is the
  name of the whole function, not of a command.
- **Operator.** A human runs the first installation; an LLM agent runs the later commands. cc-sync is a CLI with no
  TUI. Every command completes without a TTY, a flag replaces each prompt, `--json` gives each report as JSON, and
  each outcome that an operator acts on has its own exit code.
- **Catalog.** The built-in items, plus referenced files, plus the repo's `include` list, minus the repo's `exclude`
  list. The same catalog applies on every machine.
- **Remote snapshot.** One git repo whose files mirror the supported items. Secret placeholders stand for secrets, and a
  home-path token stands for the home directory.
- **Pull.** A three-way comparison per item against the base commit. The operator picks a side for each conflict, in a
  prompt or by flag. On the first pull, `pull --prefer remote` sets up a new machine in one command. Pull adds missing
  marketplaces through the `claude` CLI, backs up each file, replaces it atomically, and changes only synced values.
  Pull warns about missing executables that synced commands name.
- **Push.** Refuses when the remote branch is ahead of the base commit. Masks secrets, scans the outgoing snapshot,
  shows the list, then commits and pushes with the `git` executable.
- **Secrets.** Pull keeps the target machine's value. A missing value is asked for on a TTY, or left out and listed;
  a separate command sets it later from a TTY or stdin. No output contains a secret value.
- **Machine-local state.** The sync configuration, the base commit, and the local clone live in `~/.cc-sync/`. Pins in
  the sync configuration hold partial-sync and OS-specific values on one machine.
- **Scope of v0.1.** A proof of concept on Linux, macOS, and Windows. Memory, `agent-memory/`, and claude-mem data stay
  out.

## Resolved Questions

- Build a new tool, or extend Hudint/claudesync or zhaojiannet/cclayer? → Build. Hudint/claudesync resolves a same-key
  edit by "last writer wins"; neither tool masks a secret for entry on the target machine; Hudint/claudesync was 3 days
  old at v0.2.1.
- What does "memories" mean? → Project auto-memory, which stays a non-goal. There is no user-level `MEMORY.md`. The
  user-level `CLAUDE.md` and `rules/` are user instructions, a supported item group.
- Item granularity → key level for JSON files, file level for authored files.
- Which `~/.claude.json` keys sync? → `mcpServers`, the Global config keys, the fallback keys, and the undocumented keys
  that are preferences. `pluginPanes` and `companion` stay out.
- `agent-memory/` → out of v0.1.
- Files that synced items point to → in the catalog as referenced files. The repo's `exclude` list removes one, and its
  `include` list adds files that nothing references.
- OS-specific values → pins on each machine. No OS layers.
- Two machines changed one item → a conflict, and the operator picks a side.
- TUI or CLI? → A CLI with no TUI. An agent runs every command after the first installation, without a TTY.
- Secret entry → pull asks on a TTY, or leaves the key out and lists it. A separate command sets it later.
- Partial sync → pins in the machine-local sync configuration. A pinned item moves in neither direction.
- Executables that synced commands need → pull checks them and warns. The user manages them.
- Location of the local clone and the sync configuration → `~/.cc-sync/`.
- Command names → `init`, `status`, `push`, `pull`. "Sync" names the whole function.
- Tool name → cc-sync.
- Implementation language → Rust, one native binary per platform from a GitHub Actions matrix. Node is out
  because a machine can lack it; Go loses to `serde`, exhaustive enums, and order-preserving JSON.

## Not Yet Specified

- How the list of undocumented `~/.claude.json` keys stays current after Claude Code releases. A `status` report of the
  top-level keys that cc-sync has not classified could sharpen it.
- How the user marks a false positive of the secret scan as not secret.
- Whether list values, such as `permissions.deny`, merge instead of conflicting.
- The file layout of the remote snapshot and of `~/.cc-sync/`.
- The behavior of `claude plugin marketplace add` without a TTY on a new machine. A probe in the frame stage settles it.

## Recorded ADRs

- [1.1 Sync only the items in the catalog, one JSON key at a time](/design-docs/adr/1.1-supported-items.md)
- [1.2 Mask secrets on push and keep each machine's own value on pull](/design-docs/adr/1.2-secret-handling.md)
- [1.3 Compare each item three ways against the base commit, and stop on conflicts](/design-docs/adr/1.3-concurrent-changes.md)
- [1.4 Sync the plugin declarations and let Claude Code install the plugins](/design-docs/adr/1.4-plugin-setup.md)
- [1.5 Handle per-machine differences with pins, a home-path token, and warnings](/design-docs/adr/1.5-per-machine-differences.md)
- [1.6 Change only synced values in Claude Code files, with a backup and an atomic replace](/design-docs/adr/1.6-target-file-writes.md)
- [1.7 Run git operations through the git executable](/design-docs/adr/1.7-git-access.md)
- [1.8 Build a CLI that an agent can operate without a TTY, and no TUI](/design-docs/adr/1.8-operator-interface.md)
- [1.9 Write cc-sync in Rust](/design-docs/adr/1.9-implementation-language.md)
