# Frame: Claude Code user-level setup sync

- **Date:** 2026-11-06
- **Alignment:** `design-docs/01-claude-setup-sync.alignment.md`

cc-sync is one Rust binary crate. Its layers: the CLI (commands, text and JSON reports, exit codes), the machine-local
state in `~/.cc-sync/`, the git access, the remote snapshot, the extraction of supported items from Claude Code files,
the value rewriting (home-path token, secret placeholders), the three-way comparison, and the writes back to Claude Code
files. Every phase below crosses all of these layers for one more kind of supported item or behavior.

Shared shapes that the phases build on:

- **Item id.** `<source>:<locator>`. Sources: `settings` and `claude` take a JSON pointer (RFC 6901), for example
  `settings:/env/FOO` or `claude:/mcpServers/kartta`; `file` takes a path relative to the Claude Code directory, for
  example `file:rules/rust.md`; `marketplace` takes a name. Agents pass item ids to `--take` and `pin`.
- **Remote snapshot layout.** `settings.json` and `claude.json` hold the catalog keys of the two Claude Code files after
  value rewriting; `files/` mirrors the authored files; `plugins/marketplaces.json` holds the marketplace sources;
  `cc-sync.toml` holds the repo's `include`, `exclude`, and secret key paths.
- **Machine-local state.** `~/.cc-sync/config.toml` (remote, branch, pins), `~/.cc-sync/repo.git` (the local clone, a
  bare repo), the base commit as the ref `refs/cc-sync/base` in the local clone, and `~/.cc-sync/backups/<timestamp>/`.
- **Item state.** One enum: unchanged, local change, remote change, conflict, pinned, secret missing. Deletions are
  changes with an absent side.
- **Exit codes.** 0 success; 1 failure; 2 usage error; 3 pull stopped on conflicts; 4 push refused because the remote is
  ahead of the base commit; 5 push blocked by the secret scan.

## Phase 1: Tracer Bullet — `settings.json` keys between two machines

**Components:**

- Cargo scaffold — binary crate, `rust-toolchain.toml`, `clippy.toml`, `.claude/rules/rust.md`, GitHub Actions CI with
  a Linux, macOS, and Windows matrix.
- CLI — `init <remote>`, `status`, `push`, `pull` with `--json`, `--prefer local|remote`, and `--take <item>=<side>`;
  exit codes.
- Machine-local state — `config.toml`, the local clone, the base commit ref.
- Git access — clone, fetch, read a snapshot from a commit, commit, push, through the `git` executable.
- Items — `settings.json` top-level keys, split per entry for `env`, `enabledPlugins`, `extraKnownMarketplaces`,
  `pluginConfigs`, `skillOverrides`, and `modelSettings`.
- Comparison — three-way per item against the base commit, with the first-pull rule.
- Writes — in-place JSON edits that keep formatting and key order, a backup, an atomic replace.

**Testing strategy:**

- Learning tests first (see Learning Tests): the JSON editor and the git commands.
- Unit tests for item extraction, the comparison table (every combination of base, local, remote), and the JSON edits.
- End-to-end tests that run the binary for two simulated machines (two temporary home directories) against one bare git
  repo on disk.

**Verification gate:**

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` pass.
- The end-to-end test moves a changed key from machine A to machine B, moves a change back, and stops on a conflict with
  exit code 3. A `settings.json` with keys outside the change is byte-identical after the pull except the changed key.

**Acceptance criteria:**

- [x] `cc-sync init` on a machine with an empty remote, then `push`, creates the first remote snapshot commit.
- [x] `cc-sync pull --prefer remote` on a second machine writes the remote keys into its `settings.json`.
- [x] A key changed on both machines stops `pull` with exit code 3 and lists the item; `--take` resolves it.
- [x] `push` refuses with exit code 4 when the remote has commits after the base commit.
- [x] `status --json` prints the item list with item ids and item states.
- [x] Each pull that writes a file leaves a backup copy in `~/.cc-sync/backups/`.

## Phase 2: Secrets, home-path token, pins, executable warnings

**Components:**

- Value rewriting — secret placeholders by key-name rules and repo-declared key paths; the home-path token in JSON
  string values.
- Push — the secret scan over the whole outgoing snapshot, exit code 5.
- Pull — keeps the target's own secret value, asks on a TTY, otherwise leaves the key out and reports `secret missing`;
  a left-out key is not a local deletion.
- CLI — `secret set <item>` (TTY prompt or stdin), `pin <item>`, `unpin <item>`; pinned items in reports.
- Pull — warnings for executables that synced commands name and that are not on PATH.

**Testing strategy:**

- Unit tests for the name rules, placeholder round trips, the scan patterns, and token expansion with Windows path
  forms.
- End-to-end: a secret in `env` never reaches any object in the bare repo; machine B keeps its own value; a missing
  secret is listed, and `secret set` fills it; a pinned key moves in neither direction; `statusLine.command` gets the
  home directory of machine B.

**Verification gate:**

- Gates of Phase 1 pass, and a search of every object in the bare repo for the test secret finds nothing.

**Acceptance criteria:**

- [x] No remote snapshot commit contains a value that a name rule matched.
- [x] A token-like string outside a placeholder stops `push` with exit code 5.
- [x] `pull` without a TTY reports each missing secret and exits 0.
- [x] A pinned item keeps its local value through `push` and `pull`.

## Phase 3: Full catalog — `~/.claude.json` keys, authored files, referenced files

**Components:**

- Items — `mcpServers` (one item per server), the Global config keys, the fallback keys, and the six undocumented
  preference keys from `~/.claude.json`; authored files (`CLAUDE.md`, `rules/`, `skills/` without `skills/synced/`,
  `agents/`, `commands/`, `output-styles/`, `workflows/`, `themes/`, `keybindings.json`).
- Referenced files — paths under the Claude Code directory in synced JSON strings, and `@` imports and relative links in
  synced Markdown files.
- Repo catalog file `cc-sync.toml` — `include` and `exclude`.
- Claude Code directory — `CLAUDE_CONFIG_DIR` when set.
- Writes — file items with backup and atomic replace; file deletions.

**Testing strategy:**

- Unit tests for reference detection in JSON strings and Markdown, and for `include` and `exclude`.
- End-to-end: an MCP server, a rule file, and a file that `statusLine.command` references move from A to B; a file
  deleted on A is deleted on B; all keys of `~/.claude.json` outside the catalog are byte-identical after a pull.

**Verification gate:**

- Gates of Phase 1 pass, and the end-to-end test above passes with a `~/.claude.json` copied in shape from a real one
  (state keys, `projects`, `oauthAccount`) that none of the pushes carries into the bare repo.

**Acceptance criteria:**

- [ ] No key of `~/.claude.json` outside the catalog appears in the remote snapshot.
- [ ] A referenced file syncs, and `exclude` removes it.
- [ ] `CLAUDE_CONFIG_DIR` moves every Claude Code path that cc-sync reads and writes.

## Phase 4: Plugin setup

**Components:**

- Items — marketplace sources from `known_marketplaces.json`, one item per marketplace, without install locations.
- Pull — `claude plugin marketplace add <source>` for a marketplace that the remote added, and
  `claude plugin marketplace remove <name>` for one that the remote removed; a warning when `claude` is not on PATH.

**Testing strategy:**

- Learning test of the real `claude` CLI with a sandbox `CLAUDE_CONFIG_DIR`, without a TTY (ignored by default; needs
  the network).
- End-to-end with a fake `claude` executable on PATH that records its arguments.

**Verification gate:**

- Gates of Phase 1 pass; the fake `claude` records one `marketplace add` for each marketplace that machine B lacks.

**Acceptance criteria:**

- [ ] `installed_plugins.json` and the plugin cache never enter the remote snapshot.
- [ ] A marketplace added on A is added on B through the `claude` CLI.

## Phase 5: Distribution and documentation

**Components:**

- GitHub Actions release workflow — a tag builds the binary on Linux, macOS, and Windows runners and attaches it to a
  GitHub release.
- `README.md` — installation, the first-machine and new-machine flows, the agent contract (`--json` shapes, exit codes,
  `--take`, `secret set`), and the catalog.

**Testing strategy:**

- Workflow lint with `actionlint` when available; the workflow runs only after the GitHub repo exists.
- README commands checked against `cc-sync --help`.

**Verification gate:**

- Every command and flag in the README exists in `cc-sync --help`.

**Acceptance criteria:**

- [ ] A tag push produces three release binaries (verified once the GitHub repo exists).
- [ ] The README documents every exit code and the JSON report shape.

## Learning Tests

- `jsonc-parser` with the `cst` feature — set, insert, and remove an object property; the rest of the text stays
  byte-identical.
- `serde_json` with `preserve_order` and `arbitrary_precision` — a parse and pretty print of copies of the real
  `settings.json` and `~/.claude.json` is byte-identical apart from the trailing newline. Run on local copies; never
  print their content.
- `git` — clone of an empty bare repo, first push with `-u`, reading files of a commit with `ls-tree` and
  `cat-file --batch`, and a ref under `refs/cc-sync/`.
- `claude plugin marketplace add` — behavior without a TTY with a sandbox `CLAUDE_CONFIG_DIR`.

## Phase Sequence

```text
Phase 1 (tracer bullet, no deps)
  |
  +--> Phase 2 (depends on Phase 1)
  +--> Phase 3 (depends on Phase 1; parallel with Phase 2)
  +--> Phase 4 (depends on Phase 1; parallel with Phases 2 and 3)
          |
          v
Phase 5 (depends on Phases 2, 3, and 4)
```

Phases 2, 3, and 4 share the item model and the extraction code, so parallel agents need one owner for those files. In
an unattended build with one agent, they run in the order 2, 3, 4.

## Scope Boundaries

**In scope:** the v0.1 proof of concept: the commands `init`, `status`, `push`, `pull`, `secret set`, `pin`, and
`unpin`, the full catalog, Linux, macOS, and Windows binaries.

**Out of scope:** memory, `agent-memory/`, claude-mem data, project-level setup, automatic sync, merges inside a value,
encryption, a TUI, the home-path token inside authored files, and publication to crates.io, npm, or Homebrew.
