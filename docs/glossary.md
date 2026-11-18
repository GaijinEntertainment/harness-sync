# harness-sync Glossary

Project vocabulary. Only entries with a real risk of being mis-named are listed; the design lives in `design-docs/`. For
each term, **Avoid** lists the synonyms that are realistic naming traps — names a reviewer or agent would plausibly
reach for and get wrong.

## Harnesses and the setup

### Harness

A program whose user-level setup harness-sync syncs: Claude Code and pi, later Codex and kensai-local. Each machine
lists the harnesses that it syncs, and harness-sync never moves a value from one harness to another.

**Avoid:** agent (kensai-local is a review CLI, not a coding agent, and "agent" also names the operator that runs
harness-sync); tool (names harness-sync itself); client (suggests a server that harness-sync talks to).

### Harness crate

The crate that describes one harness to the core: its directories, its catalog, the commands that its items name, and
its own warnings. The binary compiles in each harness crate that it lists.

**Avoid:** plugin (names Claude Code plugins and pi packages, which harness-sync syncs as declarations); adapter
(suggests a conversion between harnesses, which harness-sync never does).

### User-level setup

The configuration of a harness that belongs to a user, not to a project, such as `~/.claude/settings.json`, the
configuration keys of `~/.claude.json`, `~/.pi/agent/models.json`, and the authored files of each harness. It does not
include what a harness writes for itself: history, caches, sessions, install records, credentials.

**Avoid:** global settings (Claude Code's docs use "Global config" for a small set of `~/.claude.json` keys, which is
only part of the setup); user settings (names one settings file); config (also names harness-sync's own sync
configuration).

### Global config key

A preference key that Claude Code reads only from `~/.claude.json`, as the settings reference lists under the "Global
config" scope, for example `diffTool` or `copyOnSelect`. One of the item groups that the Claude Code catalog takes from
`~/.claude.json`.

**Avoid:** global setting (reads as the whole user-level setup); user setting (Claude Code ignores these keys in
`settings.json`).

### User instructions

`~/.claude/CLAUDE.md` and the files in `~/.claude/rules/`. A group of supported items of the Claude Code catalog.

**Avoid:** memory, user memory (Claude Code's docs call `CLAUDE.md` "user memory", but in this project memory is
auto-memory, which harness-sync does not sync).

### Memory

Auto-memory: the per-project directory `~/.claude/projects/<project>/memory/` that Claude writes, with its `MEMORY.md`
index. It is not a supported item, because its content depends on projects that differ between machines. No user-level
`MEMORY.md` exists.

**Avoid:** user memory (Claude Code's name for `CLAUDE.md`, which is user instructions here); claude-mem (a third-party
plugin's own database, a different store that is also out of scope).

## Items and the catalog

### Supported item

The unit that harness-sync compares and moves: one key of a keyed JSON file (a `settings.json` key, one MCP server, one
pi provider) or one whole file. Status, push, and pull list differences per supported item.

**Avoid:** file (most items are keys inside a file); setting (items are also files and MCP servers); entry (also names
any JSON object member, synced or not).

### Item id

The name of a supported item: the home-relative path of its file with forward slashes, plus `:` and a JSON pointer for
a key, as in `.claude/settings.json:/env/EDITOR`. The same form names a value inside an item, as in the repo's `secrets`
list.

**Avoid:** key path (names only the JSON pointer part, which two files can share); repo path (the path of the file in
the repo, without the key).

### Catalog

The set of supported items of one harness, the same on every machine: its built-in item list, plus referenced files,
plus the repo's `include` list, minus the repo's `exclude` list. The repo stores `include` and `exclude`.

**Avoid:** sync configuration (machine-local, while a catalog change reaches every machine); allowlist (names the
mechanism, not the set); manifest (suggests a list of installed packages).

### Referenced file

A file under a harness directory that a synced item points to: a path in a synced JSON string value, or an `@` import
or a relative link in a synced Markdown file. It is in the catalog unless the repo's `exclude` list removes it.

**Avoid:** dependency (suggests executables such as `node` or `jq`, which harness-sync does not sync); extra file (an
`include` entry, which needs no reference).

### Root file

A file at a home-relative path that no harness owns, which the repo's `include` list adds, such as a shared
instructions file or a file in `~/.agents/skills/`. It syncs on every machine, whatever harnesses the machine lists.

**Avoid:** shared file (suggests that harness-sync shares it between harnesses, but it is a plain file that harnesses
may read on their own); root setup (suggests a harness of its own, with a catalog).

## Repo and machine state

### Remote snapshot

The synced setup as a target stores it at its latest commit. Status, pull, and the base commit use the remote snapshot
of the primary target. Secret placeholders stand for secrets, and a home-path token stands for the home directory.

**Avoid:** backup (implies a one-way restore, but every machine pushes); master copy (no machine and no copy is
authoritative); remote (the git remote, which holds the snapshot but is not the snapshot).

### Unmanaged file

A file in the repo of a target that is not `harness-sync.toml`, that no harness owns, and that no `include` entry
covers, such as a `README.md` or a file in the layout of an earlier tool. A file that never syncs, such as a
credentials file, is also unmanaged. Pull never writes one to the home directory, status and push warn about each, and
`push --prune` removes them.

**Avoid:** root file (a home file that `include` adds, which harness-sync does manage); stray file, dirty remote (a
`README.md` that the user keeps in the repo is unmanaged and wanted); untracked file (git's name for a file outside
the index, but git tracks these files).

### Base commit

The remote snapshot commit of the primary target that this machine last reconciled its setup with. A pull can keep
local changes, so the setup can differ from it. A successful pull and a successful push to the primary target both set
it, and making another target primary drops it. It is the common ancestor in the three-way comparison of each supported
item.

**Avoid:** last pulled commit (a push also moves it); last sync (sync names the whole function, not one event).

### Sync configuration

The machine-local configuration in `~/.harness-sync/`: the harnesses of this machine, the targets, the primary target,
and this machine's pins. Push never sends it.

**Avoid:** config (collides with the configuration of each harness); catalog (shared through the repo with every
machine); profile (other sync tools use it for shared layers that harness-sync does not have).

### Target

A named place in the sync configuration that push can send the snapshot to: a repo and a branch for the git and Gerrit
storage kinds. Each machine names its own targets, and one of them is the primary target. A target is never the machine
that a pull writes to.

**Avoid:** remote (the git remote is only the URL of a target; a target also names a branch and a storage kind);
mirror (implies a copy of the primary, but a target's own `harness-sync.toml` can make its snapshot a subset).

### Storage kind

The kind of storage behind a target, which a storage crate implements: `git` or `gerrit`. It decides how a commit is
published and whether the publish moves the base commit.

**Avoid:** backend (suggests a server process); provider (pi and kensai-local name their LLM services so); flavor (the
sync configuration names git and Gerrit as two kinds, not as variants of one).

### Primary target

The target that status and pull read, that the base commit belongs to, and that push sends to by default. Each machine
picks its own primary target.

**Avoid:** origin (the default name of the first target, which a later switch leaves as a plain target); main
(collides with the branch name).

### Pin

A machine-local mark on a supported item: the item keeps its local value, and neither push nor pull moves it. Partial
sync and OS-specific values both use pins.

**Avoid:** exclude (removes an item from the catalog on every machine); override (implies a value that comes from a
layer, and harness-sync has no layers).

## Commands and comparison

### Operator

The human or the LLM agent that runs a harness-sync command. A human runs the first installation; an agent runs the
later commands, without a TTY.

**Avoid:** user (the human who owns the user-level setup; an agent operates harness-sync for that human, so a rule
written for "the user" silently excludes the agent).

### Pull

The command that brings the remote snapshot to this machine. Its counterpart is push. Sync is the name of the whole
function of harness-sync, not of a command.

**Avoid:** sync (one name for two concepts: the function and the command); restore (implies a one-way backup).

### Conflict

A supported item that changed both locally and in the remote snapshot since the base commit, to different values. Pull
stops on it, and the operator picks the local or the remote value. Without a base commit, every item that both sides
hold with different values is a conflict.

**Avoid:** difference (any item where the local value and the remote snapshot differ; status lists differences, and
only some of them are conflicts); merge conflict (git's term for a clash of text lines; harness-sync compares items,
not lines).

## Value rewriting

### Secret placeholder

The named marker that replaces a secret value in the remote snapshot. For it, pull keeps the value that the machine
that pulls already has.

**Avoid:** token (also names the auth tokens that are the secrets, and the home-path token); mask (the act of replacing,
not the stored marker); redacted value (suggests the value is gone, but each machine keeps its own).

### Home-path token

The marker that replaces the home directory prefix of a path in the remote snapshot. Pull expands it to the home
directory of the machine that pulls.

**Avoid:** variable (suggests `${VAR}` environment expansion, which Claude Code and pi do themselves in some values);
placeholder (names the secret marker).
