# harness-sync

harness-sync keeps the user-level setup of Claude Code and pi on several machines in step through a git repo you own.
Each machine pushes its own changes and pulls the changes of the others; no machine is the master. A human runs the
first setup, and after that an LLM agent can run every command without a terminal, reading JSON reports and exit codes.

The design and the decisions behind it are in [`design-docs/`](design-docs/).

## What syncs

Each harness syncs on its own; harness-sync never moves a value from one harness to another.

### Claude Code

- `~/.claude/settings.json`, one item per top-level key. The entries of `env`, `enabledPlugins`,
  `extraKnownMarketplaces`, `pluginConfigs`, `skillOverrides`, and `modelSettings` are one item each.
- From `~/.claude.json`: each MCP server in `mcpServers`, the Global config keys of the Claude Code settings reference,
  the settings keys that Claude Code reads from `~/.claude.json` when no settings file sets them, and the preference
  keys `showSpinnerTree`, `diffSidebarOpen`, `deepLinkTerminal`, `lspRecommendationDisabled`, and `companionMuted`.
- The authored files under `~/.claude/`: `CLAUDE.md`, `keybindings.json`, and the files in `rules/`, `skills/`,
  `agents/`, `commands/`, `output-styles/`, `workflows/`, and `themes/`. A file keeps its executable bit.
- Referenced files: a file under `~/.claude/` that a synced JSON string names, such as a status line script or a hook
  script, and a file that a synced Markdown file imports with `@` or links with a relative link.
- The plugin setup, as the settings entries above: `enabledPlugins`, `extraKnownMarketplaces`, and `pluginConfigs`.
  harness-sync runs no Claude Code command. After its next session starts, Claude Code clones each declared marketplace
  that it lacks and downloads the enabled plugins. A marketplace that settings do not declare does not sync: status
  and push warn about it, unless the repo's `exclude` list covers it, and show the entry to add to
  `extraKnownMarketplaces`.

Everything else stays on the machine: the rest of `~/.claude.json` (counters, caches, account data, the `projects`
records), credentials, history, sessions, auto-memory, `skills/synced/`, the removed skills in `skills/.trash/`, the
plugin install records, the plugin cache, and every `node_modules` directory. When `CLAUDE_CONFIG_DIR` is set,
harness-sync reads and writes the Claude Code files in that directory, as Claude Code does.

### pi

- `~/.pi/agent/settings.json`, one item per top-level key, except the state keys `lastChangelogVersion`, `trackingId`,
  and `deviceId`. The package declarations sync as the `packages`, `extensions`, `skills`, `prompts`, and `themes`
  keys; pi installs the declared packages itself.
- `models.json`, one item per provider in `providers`, and `mcp.json`, one item per server in `mcpServers`.
- `keybindings.json`, the context files `AGENTS.override.md`, `AGENTS.md`, and `CLAUDE.md`, `SYSTEM.md`,
  `APPEND_SYSTEM.md`, and the files in `extensions/`, `skills/`, `prompts/`, and `themes/`.

The credentials in `auth.json`, `mcp-auth.json`, and `models-store.json`, the sessions, the npm tree, and the caches
stay on the machine. When `PI_CODING_AGENT_DIR` is set, harness-sync reads and writes the pi files in that directory.

### Other files

A file or directory under the home directory that no harness owns, such as `~/.agents/skills/`, syncs when the repo's
`include` list names it; see [`harness-sync.toml`](#harness-synctoml).

## Install

Download the archive for your platform from the GitHub releases of this repo and put `harness-sync` on your PATH, or
build it with Rust 1.99 or later:

```sh
cargo install --locked --git https://github.com/xobotyi/harness-sync harness-sync
```

harness-sync runs every repo operation through the `git` executable, so `git` must be on PATH with access to your
remote.

## Set up the first machine

Create an empty private repo on any git host, then connect the machine and send its setup:

```sh
harness-sync init git@github.com:you/agent-setup.git
harness-sync push
```

`init` takes `--branch` when the snapshot should live on a branch other than `main`, and `--name` to name the repo
something other than `origin`; see [Targets](#targets). It syncs each harness whose directory exists on the machine;
`--harness claude-code` names the harnesses instead, and `harness-sync harness add|remove|list` changes the list later.
The machine-local state lives in `~/.harness-sync/`: `config.toml` (the harnesses, the targets, the primary target, and
the pins of this machine), `repo.git` (a bare clone), `missing-secrets.toml`, and `backups/`.

A machine reads and writes only the items of the harnesses it syncs. Push leaves the items of other harnesses in the
repo as they are. A harness that a machine starts to sync after its last pull has no base commit until the next pull:
push refuses, and pull treats each of its items that both sides hold with different values as a conflict.

## Set up another machine

```sh
harness-sync init git@github.com:you/agent-setup.git
harness-sync pull --prefer remote
```

On a machine that has never pulled, every item that both sides hold with different values is a conflict, so
`--prefer remote` takes the remote value for all of them. An item that only one side holds comes from that side. Secrets
do not travel; see [Secrets](#secrets) for how the new machine gets its own values.

## Targets

A machine can keep its setup in several git repos. Each repo is a target with a name, and one target is the primary:
status and pull read it, and push sends to it. `init` makes the first repo the primary target. Each machine keeps its
own list in `~/.harness-sync/config.toml`, because the repos a machine can reach differ. Reports print target URLs, so
`init` and `target add` refuse a URL that holds a password; keep the password in a git credential helper.

```sh
harness-sync target add mirror git@example.com:you/agent-setup.git --branch master
harness-sync target list
harness-sync target primary mirror
harness-sync target remove mirror
```

A new primary target has no base commit on this machine, so the first pull from it stops on each item that both sides
hold with different values: settle them with `--prefer` or `--take`.

`harness-sync push --to mirror` sends to that target, and `--to all` sends to every target, the primary first. A target
that is not the primary gets this machine's setup as its new snapshot, without a conflict check, and with the `secrets`,
`include`, and `exclude` of its own `harness-sync.toml`. So a target can hold a subset of the setup. Push stops at the
first target that refuses, for example a primary with commits this machine has not pulled.

A Gerrit project takes changes for review in place of direct pushes. Add it with `--gerrit`, which `init` also takes.
Push then sends the snapshot as a change to `refs/for/<branch>`, with a `Change-Id` line, and reports the change link.
harness-sync never merges the change. The branch of the target, and the base commit of a Gerrit primary, move after you
merge it; until then status lists the pushed items as local changes, and each push opens a new change.

## Day to day

`harness-sync status` lists the items that differ from the remote snapshot. `harness-sync pull` applies the changes of
the other machines and keeps the changes of this one. `harness-sync push` sends the changes of this machine.

Push refuses with exit code 4 when the remote has commits that this machine has not pulled: run `harness-sync pull`,
then push again. Each pull that changes a file first copies it into `~/.harness-sync/backups/<timestamp>/`. In a JSON
file, pull changes only the synced values and leaves every other byte as it was.

## Conflicts

harness-sync compares each item three ways: the local value, the remote snapshot, and the base commit, which is the
snapshot this machine last pulled or pushed. A change on one side moves to the other. A change on both sides to
different values is a conflict, and pull stops with exit code 3 before it writes anything.

Choose a side per item with `--take`, or for every remaining conflict with `--prefer`:

```sh
harness-sync pull --take .claude/settings.json:/model=remote --prefer local
```

When stdin and stdout are a terminal, pull asks for each conflict that no flag settles. `--no-input` turns every prompt
off.

## Secrets

Push replaces each secret string with a placeholder such as
`<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>`, so the repo never holds the value. A placeholder counts
only at the value whose item id it names; the same text anywhere else is a plain string. A string is a secret
when a key on its path, in any case, is `Authorization` or ends with `_KEY`, `-KEY`, `APIKEY`, `TOKEN`, `SECRET`,
`PASSWORD`, or `HEADERS`. The repo can declare more key paths; see
[`harness-sync.toml`](#harness-synctoml).

Before it commits, push scans the outgoing snapshot, authored files included, for token-like strings outside a
placeholder, such as a GitHub, Anthropic, Slack, or AWS key or a private key block. A match stops the push with exit
code 5 and lists the key paths in `secrets_found`. Reports never print a secret value.

Pull keeps the value that this machine already has for each placeholder. When the machine has no value, pull asks for it
on a terminal; without one, pull leaves the key out and lists it as `secret_missing`. A missing secret inside a JSON
array leaves the whole array out, and `secret_missing` lists the array; edit the harness file to set it. A missing
secret is not a deletion: push keeps the placeholder until the local setup holds a value at the key. Set the value later
from a prompt or from stdin:

```sh
printf '%s' "$TOKEN" | harness-sync secret set .claude/settings.json:/env/GITHUB_TOKEN
```

## Per-machine differences

A path under the home directory in a synced JSON value travels as `<harness-sync home>` and arrives as the home
directory of the machine that pulls it, with forward slashes on Windows.

`harness-sync pin <item>` keeps an item as it is on this machine: push sends the remote value instead, and pull leaves
the local value. Use it for a value only one machine needs, or for an OS-specific value such as a Windows status line.
`harness-sync unpin <item>` lets the item move again.

Pull warns when a synced status line, hook, or MCP server names a program that is not installed. It installs nothing.

## harness-sync.toml

Each repo can hold a `harness-sync.toml` at its root with the settings of that repo. A machine reads it from the remote
snapshot of its primary target, and a push to another target applies that target's own file. Change it from any
machine; the command commits the new file to the repo, or opens a change for a Gerrit target:

```sh
harness-sync declarations add exclude .claude/settings.json:/extraKnownMarketplaces/yatai
harness-sync declarations remove exclude .claude/settings.json:/model --target mirror
harness-sync declarations show
```

The command rewrites the file and drops its comments. You can also edit the file in any clone of the repo; push keeps
the files it does not manage. The file looks like this:

```toml
secrets = [".claude/settings.json:/env/INTERNAL_WEBHOOK"]
include = [".claude/hooks", ".claude.json:/autoCompactEnabled"]
exclude = [".claude/statusline-mac.sh", ".claude/settings.json:/model"]
```

- `secrets` marks every string under each key path as a secret.
- `include` adds files, whole directories, and keys of the JSON files that sync key by key, such as `~/.claude.json`. A
  nested key in a key that the catalog leaves out, such as `.claude.json:/projects/~1work~1app/allowedTools`, syncs
  without the rest of that key. A file or directory that no harness owns syncs too, at its path under the home
  directory. `include` never adds a credentials file of a harness, `~/.claude/skills/synced/` or
  `~/.claude/skills/.trash/`, a file in `~/.harness-sync/`, or a `node_modules` directory or `.DS_Store` file. A
  credentials file that no harness lists, such as `~/.aws/credentials`, syncs when `include` names it.
- `exclude` removes every item under each entry, on every machine. `harness-sync declarations add exclude` also removes
  those items from the repo in the same commit.

A secret that a push sent before its key path was declared stays in the git history of the repo; the next push replaces
it with a placeholder.

## Operating harness-sync from an agent

Every command runs without a terminal. Pass `--json` for a machine-readable report on stdout and `--no-input` to rule
out prompts. Errors go to stderr as `error: ...`.

| Exit code | Meaning                                                   |
| --------- | --------------------------------------------------------- |
| 0         | Success                                                   |
| 1         | Failure; the message is on stderr                         |
| 2         | Usage error                                               |
| 3         | Pull stopped on conflicts and wrote nothing               |
| 4         | Push refused: the remote has commits this machine lacks   |
| 5         | Push blocked: the secret scan found a token-like value    |

A report looks like this:

```json
{
  "command": "status",
  "outcome": "ok",
  "base_commit": null,
  "remote_commit": "c7f9a769115a0e7872e918fa2504468b6c12b22f",
  "items": [
    {
      "id": ".claude/settings.json:/env/GITHUB_TOKEN",
      "state": "remote_change",
      "remote": "<harness-sync secret .claude/settings.json:/env/GITHUB_TOKEN>"
    },
    {
      "id": ".claude/settings.json:/model",
      "state": "conflict",
      "local": "sonnet",
      "remote": "opus"
    }
  ]
}
```

- `outcome` is `ok`, `conflicts`, `remote_ahead`, or `secrets_found`.
- `state` is `local_change`, `remote_change`, `conflict`, `pinned`, or `secret_missing`. An item leaves out `local` or
  `remote` when that side does not hold it; `null` is a JSON `null` value.
- `secrets_found` lists key paths when the scan blocks a push, `missing_executables` lists `{id, executable}` pairs
  after a pull, and `undeclared_marketplaces` lists `{name, declaration}` pairs from status and push for the
  marketplaces that do not sync and that the repo's `exclude` list does not cover. Each appears only when it holds
  entries.
- `pushes` lists `{target, outcome, commit, review, items}` for each target that a push reached; `review` holds
  the pushed `commit` and the `change_url` for a Gerrit target. The top-level `items`,
  `base_commit`, and `remote_commit` belong to the primary target; the `items` of another target list what the push
  replaced there. `targets` lists `{name, url, branch, kind, primary}` for `target list`, `harnesses` lists
  `{name, synced}` for `harness list`, and `declarations` lists `{target, secrets, include, exclude}` for the
  `declarations` commands.
- A file item shows its git blob id and executable bit in place of its content.

Item ids name what `--take`, `pin`, and `unpin` act on. An item id is the path of the file relative to the home
directory, and for a JSON key the path, `:`, and a JSON pointer:

- `.claude/settings.json:/env/EDITOR`
- `.claude.json:/mcpServers/kartta`
- `.claude/rules/rust.md`

The repo holds each file at the same path, so `.claude/settings.json` in the repo holds the synced keys of
`~/.claude/settings.json`. A key path for `secret set` has the same form as an item id and can point deeper into an
item.

## Limitations

- The home directory in the content of an authored file is not rewritten; only JSON values get the home-path token.
- harness-sync never reads or writes through a symlink below the home directory, other than the directory that
  `CLAUDE_CONFIG_DIR` or `PI_CODING_AGENT_DIR` names. A file behind one does not sync on that machine, such as each file
  of a `~/.claude/skills/` that a dotfiles manager links, and the repo keeps it.
- A file whose path holds `\`, `:`, or a line break does not sync.
- An array or object that pull adds to a JSON file gets the formatting of the JSON editor, and an added array is written
  on one line.
- On Windows the executable bit of a file comes from the remote snapshot.

## License

MIT
