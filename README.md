# cc-sync

cc-sync keeps the Claude Code user-level setup of several machines in step through a git repo you own. Each machine
pushes its own changes and pulls the changes of the others; no machine is the master. A human runs the first setup, and
after that an LLM agent can run every command without a terminal, reading JSON reports and exit codes.

The design and the decisions behind it are in [`design-docs/`](design-docs/).

## What syncs

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
  cc-sync runs no Claude Code command. After its next session starts, Claude Code clones each declared marketplace
  that it lacks and downloads the enabled plugins. A marketplace that settings do not declare does not sync: status
  and push warn about it and show the entry to add to `extraKnownMarketplaces`.

Everything else stays on the machine: the rest of `~/.claude.json` (counters, caches, account data, the `projects`
records), credentials, history, sessions, auto-memory, `skills/synced/`, the plugin install records, and the plugin
cache. When `CLAUDE_CONFIG_DIR` is set, cc-sync reads and writes the Claude Code files in that directory, as Claude Code
does.

## Install

Download the archive for your platform from the GitHub releases of this repo and put `cc-sync` on your PATH, or build it
with Rust 1.99 or later:

```sh
cargo install --locked --git https://github.com/xobotyi/cc-sync
```

cc-sync runs every repo operation through the `git` executable, so `git` must be on PATH with access to your remote.

## Set up the first machine

Create an empty private repo on any git host, then connect the machine and send its setup:

```sh
cc-sync init git@github.com:you/claude-setup.git
cc-sync push
```

`init` takes `--branch` when the snapshot should live on a branch other than `main`, and `--name` to name the repo
something other than `origin`; see [Targets](#targets). The machine-local state lives in `~/.cc-sync/`:
`config.toml` (the targets, the primary target, and the pins of this machine), `repo.git` (a bare clone),
`missing-secrets.toml`, and `backups/`.

## Set up another machine

```sh
cc-sync init git@github.com:you/claude-setup.git
cc-sync pull --prefer remote
```

On a machine that has never pulled, every item that both sides hold with different values is a conflict, so
`--prefer remote` takes the remote value for all of them. An item that only one side holds comes from that side. Secrets
do not travel; see [Secrets](#secrets) for how the new machine gets its own values.

## Targets

A machine can keep its setup in several git repos. Each repo is a target with a name, and one target is the primary:
status and pull read it, and push sends to it. `init` makes the first repo the primary target. Each machine keeps its
own list in `~/.cc-sync/config.toml`, because the repos a machine can reach differ.

```sh
cc-sync target add mirror git@example.com:you/claude-setup.git --branch master
cc-sync target list
cc-sync target primary mirror
cc-sync target remove mirror
```

A new primary target has no base commit on this machine, so the first pull from it stops on each item that differs:
settle them with `--prefer` or `--take`.

## Day to day

`cc-sync status` lists the items that differ from the remote snapshot. `cc-sync pull` applies the changes of the other
machines and keeps the changes of this one. `cc-sync push` sends the changes of this machine.

Push refuses with exit code 4 when the remote has commits that this machine has not pulled: run `cc-sync pull`, then
push again. Each pull that changes a file first copies it into `~/.cc-sync/backups/<timestamp>/`. In a JSON file, pull
changes only the synced values and leaves every other byte as it was.

## Conflicts

cc-sync compares each item three ways: the local value, the remote snapshot, and the base commit, which is the snapshot
this machine last pulled or pushed. A change on one side moves to the other. A change on both sides to different values
is a conflict, and pull stops with exit code 3 before it writes anything.

Choose a side per item with `--take`, or for every remaining conflict with `--prefer`:

```sh
cc-sync pull --take settings:/model=remote --prefer local
```

When stdin and stdout are a terminal, pull asks for each conflict that no flag settles. `--no-input` turns every prompt
off.

## Secrets

Push replaces each secret string with a placeholder such as `<cc-sync secret settings:/env/GITHUB_TOKEN>`, so the repo
never holds the value. A string is a secret when a key on its path, in any case, is `Authorization` or ends with
`TOKEN`, `SECRET`, `PASSWORD`, `HEADERS`, or `_KEY`. The repo can declare more key paths; see
[`cc-sync.toml`](#cc-synctoml).

Before it commits, push scans the outgoing snapshot, authored files included, for token-like strings outside a
placeholder, such as a GitHub, Anthropic, Slack, or AWS key or a private key block. A match stops the push with exit
code 5 and lists the key paths in `secrets_found`. Reports never print a secret value.

Pull keeps the value that this machine already has for each placeholder. When the machine has no value, pull asks for it
on a terminal; without one, pull leaves the key out and lists it as `secret_missing`. A missing secret is not a
deletion: the next push keeps the placeholder. Set the value later from a prompt or from stdin:

```sh
printf '%s' "$TOKEN" | cc-sync secret set settings:/env/GITHUB_TOKEN
```

## Per-machine differences

A path under the home directory in a synced JSON value travels as `<cc-sync home>` and arrives as the home directory of
the machine that pulls it, with forward slashes on Windows.

`cc-sync pin <item>` keeps an item as it is on this machine: push sends the remote value instead, and pull leaves the
local value. Use it for a value only one machine needs, or for an OS-specific value such as a Windows status line.
`cc-sync unpin <item>` lets the item move again.

Pull warns when a synced status line, hook, or MCP server names a program that is not installed. It installs nothing.

## cc-sync.toml

The repo can hold a `cc-sync.toml` at its root. Every machine reads it from the remote snapshot. Edit it in any clone of
the repo; push keeps the files it does not manage.

```toml
secrets = ["settings:/env/INTERNAL_WEBHOOK"]
include = ["file:hooks", "claude:/autoCompactEnabled"]
exclude = ["file:statusline-mac.sh", "settings:/model"]
```

- `secrets` marks every string under each key path as a secret.
- `include` adds files, whole directories, and `~/.claude.json` keys to the catalog. It never adds the credentials file.
- `exclude` removes every item under each entry, on every machine.

## Operating cc-sync from an agent

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
      "id": "settings:/env/GITHUB_TOKEN",
      "state": "remote_change",
      "local": null,
      "remote": "<cc-sync secret settings:/env/GITHUB_TOKEN>"
    },
    {
      "id": "settings:/model",
      "state": "conflict",
      "local": "sonnet",
      "remote": "opus"
    }
  ]
}
```

- `outcome` is `ok`, `conflicts`, `remote_ahead`, or `secrets_found`.
- `state` is `local_change`, `remote_change`, `conflict`, `pinned`, or `secret_missing`. `null` means the side does not
  hold the item.
- `secrets_found` lists key paths when the scan blocks a push, `missing_executables` lists `{id, executable}` pairs
  after a pull, and `undeclared_marketplaces` lists `{name, declaration}` pairs from status and push for the
  marketplaces that do not sync. Each appears only when it holds entries.
- A file item shows its git blob id and executable bit in place of its content.

Item ids name what `--take`, `pin`, and `unpin` act on:

- `settings:/<JSON pointer>`, for example `settings:/env/EDITOR`
- `claude:/<JSON pointer>`, for example `claude:/mcpServers/kartta`
- `file:<path under ~/.claude>`, for example `file:rules/rust.md`

A key path for `secret set` has the same form as an item id and can point deeper into an item.

## Limitations

- The home directory in the content of an authored file is not rewritten; only JSON values get the home-path token.
- Symlinks under the Claude Code directory are skipped.
- An array or object that pull adds to a JSON file gets the formatting of the JSON editor, and an added array is written
  on one line.
- On Windows the executable bit of a file comes from the remote snapshot.

## License

MIT
