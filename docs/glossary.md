# cc-sync Glossary

Project vocabulary. Only entries with a real risk of being mis-named are listed; the design lives in `design-docs/`. For
each term, **Avoid** lists the synonyms that are realistic naming traps — names a reviewer or agent would plausibly
reach for and get wrong.

## The setup

### User-level setup

The Claude Code configuration that belongs to a user, not to a project: `~/.claude/settings.json`, the configuration
keys of `~/.claude.json`, the authored files under `~/.claude/`, and the plugin setup. It does not include what Claude
Code writes for itself: history, caches, sessions, install records, credentials.

**Avoid:** global settings (Claude
Code's docs use "Global config" for a small set of `~/.claude.json` keys, which is only part of the setup); user
settings (names only `~/.claude/settings.json`); config (also names the tool's own sync configuration).

### Global config key

A preference key that Claude Code reads only from `~/.claude.json`, as the settings reference lists under the "Global
config" scope, for example `diffTool` or `copyOnSelect`. One of the item groups that cc-sync takes from
`~/.claude.json`.

**Avoid:** global setting (reads as the whole user-level setup); user setting (Claude Code ignores
these keys in `settings.json`).

### User instructions

`~/.claude/CLAUDE.md` and the files in `~/.claude/rules/`. A group of supported items.

**Avoid:** memory, user memory
(Claude Code's docs call `CLAUDE.md` "user memory", but in this project memory is auto-memory, which cc-sync does not
sync).

### Memory

Auto-memory: the per-project directory `~/.claude/projects/<project>/memory/` that Claude writes, with its `MEMORY.md`
index. It is not a supported item, because its content depends on projects that differ between machines. No user-level
`MEMORY.md` exists.

**Avoid:** user memory (Claude Code's name for `CLAUDE.md`, which is user instructions here);
claude-mem (a third-party plugin's own database, a different store that is also out of scope).

## Items and the catalog

### Supported item

The unit that cc-sync compares and moves: one key of a JSON file (a `settings.json` key, one MCP server, one
`~/.claude.json` preference key) or one authored file. Status, push, and pull list differences per supported item.

**Avoid:** file (most items are keys inside a file); setting (items are also files and MCP servers); entry (also names
any JSON object member, synced or not).

### Catalog

The set of supported items, the same on every machine: the built-in item list, plus referenced files, plus the repo's
`include` list, minus the repo's `exclude` list. The repo stores `include` and `exclude`.

**Avoid:** sync configuration
(machine-local, while a catalog change reaches every machine); allowlist (names the mechanism, not the set); manifest
(suggests a list of installed packages).

### Referenced file

A file under `~/.claude/` that a synced item points to: a path in a synced JSON string value, or an `@` import or a
relative link in a synced Markdown file. It is in the catalog unless the repo's `exclude` list removes it.

**Avoid:**
dependency (suggests executables such as `node` or `jq`, which cc-sync does not sync); extra file (an `include` entry,
which needs no reference).

## Repo and machine state

### Remote snapshot

The user-level setup as stored at the head of the repo's branch on the git remote. Secret placeholders stand for
secrets, and a home-path token stands for the home directory.

**Avoid:** backup (implies a one-way restore, but every
machine pushes); master copy (no machine and no copy is authoritative); remote (the git remote, which holds the snapshot
but is not the snapshot).

### Base commit

The remote snapshot commit that this machine last reconciled its setup with. A pull can keep local changes, so the setup
can differ from it. A successful pull and a successful push both set it. It is the common ancestor in the three-way
comparison of each supported item.

**Avoid:** last pulled commit (a push also
moves it); last sync (sync names the whole function, not one event).

### Sync configuration

The machine-local configuration in `~/.cc-sync/`: the remote and this machine's pins. Push never sends it.

**Avoid:**
config (collides with Claude Code's configuration); catalog (shared through the repo with every machine); profile (other
sync tools use it for shared layers that cc-sync does not have).

### Pin

A machine-local mark on a supported item: the item keeps its local value, and neither push nor pull moves it. Partial
sync and OS-specific values both use pins.

**Avoid:** exclude (removes an item from the catalog on every machine);
override (implies a value that comes from a layer, and cc-sync has no layers).

## Commands and comparison

### Operator

The human or the LLM agent that runs a cc-sync command. A human runs the first installation; an agent runs the later
commands, without a TTY.

**Avoid:** user (the human who owns the user-level setup; an agent operates cc-sync for that human, so a rule written
for "the user" silently excludes the agent).

### Pull

The command that brings the remote snapshot to this machine. Its counterpart is push. Sync is the name of the whole
function of cc-sync, not of a command.

**Avoid:** sync (one name for two concepts: the function and the command);
restore (implies a one-way backup).

### Conflict

A supported item that changed both locally and in the remote snapshot since the base commit, to different values. Pull
stops on it, and the operator picks the local or the remote value. Without a base commit, every item that differs is a
conflict.

**Avoid:** difference (any item where the local value and the remote snapshot differ; status lists
differences, and only some of them are conflicts); merge conflict (git's term for a clash of text lines; cc-sync
compares items, not lines).

## Value rewriting

### Secret placeholder

The named marker that replaces a secret value in the remote snapshot. For it, pull keeps the value that the machine
that pulls already has.

**Avoid:** token (also names the auth tokens that are the secrets, and the home-path token); mask (the act of replacing,
not the stored marker); redacted value (suggests the value is gone, but each machine keeps its own).

### Home-path token

The marker that replaces the home directory prefix of a path in the remote snapshot. Pull expands it to the home
directory of the machine that pulls.

**Avoid:** variable (suggests `${VAR}` environment expansion, which Claude Code does
itself in MCP definitions); placeholder (names the secret marker).
