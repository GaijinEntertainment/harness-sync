# Frame: harness-sync

- **Date:** 2026-11-07
- **Alignment:** `design-docs/01-harness-sync.alignment.md`

The phases change one binary crate in place until Phase 5, which moves the finished seams into separate crates. Each
phase ends with the gates of `.claude/rules/rust.md` and an end-to-end test against a bare repo on disk.

## Phase 1: Tracer Bullet — Claude Code through a harness description, with path ids and the home mirror

**Components:**

- `harness` module — a `Harness` description: name, directories (with the environment variable that moves each), keyed
  JSON files with their key rules, whole files, catalog directories, skipped directories, credentials files, the
  commands that its items name. `claude_code` is its only value.
- `item` — an item id is a home-relative path, plus `:` and a JSON pointer for a key. The text form parses and prints
  `.claude/settings.json:/env/EDITOR`. `JsonFile` and the `settings:`, `claude:`, `file:` forms go.
- `setup`, `files`, `references`, `paths` — read the catalog from the harness description; the remote snapshot mirrors
  the home directory, with no `files/` folder.
- `secret`, `home`, `pin`, `declarations`, `declare`, `pull`, `push`, `output` — take the new ids.
- `executables`, `marketplaces` — read the command locations and the marketplace check from the Claude Code harness.
- `tests/e2e` — fixtures and helpers write and read the new layout and ids.

**Testing strategy:**

- Refactor first, behavior identical: the harness description replaces the constants with the existing tests green.
- Then the id and layout change: unit tests of the id text form (paths with `:`, pointers with `~1`, Windows
  separators), and end-to-end tests that read files at `.claude/settings.json`, `.claude.json`, `.claude/rules/…` in the
  bare repo.

**Verification gate:**

- A push from one machine and a pull on a second machine move a setting, an MCP server, a `~/.claude.json` key, and a
  rule file; the bare repo holds them at their home-relative paths.

**Acceptance criteria:**

- [ ] No module outside `harness/claude_code.rs` names a Claude Code path or key.
- [ ] `harness-sync.toml`, pins, `--take`, and reports use path ids.
- [ ] The remote snapshot has no `settings.json`, `claude.json`, or `files/` at its root.

## Phase 2: Harness lists per machine, and root files

**Components:**

- `config` — `harnesses` list in the sync configuration.
- `init` — `--harness <name>` (repeatable); without it, each compiled-in harness whose directory exists.
- New `harness add|remove|list` commands.
- `session`, `setup` — load the items of the listed harnesses and the root files only; push writes a snapshot that
  keeps the paths of unlisted harnesses as they are.
- `declarations` — an `include` entry outside every harness directory adds root files.
- 1.3 rule: a harness added after the base commit has no base until the next pull; push refuses until then.

**Testing strategy:**

- End-to-end: a machine without a harness pulls and leaves that harness's files alone; its push keeps them in the repo.
- End-to-end: `harness add` on a machine whose base commit predates it; push refuses, pull reports conflicts for
  differing items, push then works.
- End-to-end: an `include` of a home-root file syncs it to a machine with another harness list.

**Verification gate:**

- Two machines with different harness lists push and pull in turn, and no item of a harness leaves the repo because one
  machine does not list it.

**Acceptance criteria:**

- [ ] `harness-sync harness list --json` reports the listed and the compiled-in harnesses.
- [ ] ADR 1.5 and ADR 1.3 pending lines removed.

## Phase 3: The pi harness and the secret name rule

**Components:**

- `harness/pi.rs` — the pi catalog of ADR 1.13, `PI_CODING_AGENT_DIR`, MCP command locations.
- `secret` — the name rule of ADR 1.2 (`-KEY`, `APIKEY`).
- `references` — references in pi JSON values resolve against the pi agent directory.

**Testing strategy:**

- Unit: the name rule on `apiKey`, `api-key`, `X-API-KEY`, `apiKeyHelper`, `MAX_THINKING_TOKENS`.
- End-to-end: pi `settings.json` (without its three state keys), `models.json` providers with an `apiKey` masked,
  `mcp.json` servers, `keybindings.json`, `extensions/`; a machine with only Claude Code ignores them.

**Verification gate:**

- A machine with Claude Code and pi and a machine with pi only exchange a pi provider and a Claude Code setting through
  one repo; the pi `apiKey` reaches the repo only as a secret placeholder.

**Acceptance criteria:**

- [ ] ADR 1.1, 1.2, 1.4, and 1.13 pending lines removed.

## Phase 4: The storage trait

**Components:**

- `storage` module — the trait of ADR 1.16: fetch, snapshot of a commit, commit of file changes on a parent, publish
  with an outcome, base commit record.
- `git` — implements it for the `git` and `gerrit` kinds; `gerrit` functions move behind the kind.
- `push`, `declare`, `pull`, `session`, `target`, `init`, `output` — call the trait; the kind branches leave them.

**Testing strategy:**

- Refactor, behavior identical: the existing end-to-end tests for push, pull, targets, declarations, and Gerrit stay
  green without edits.

**Verification gate:**

- No command module matches on the target kind.

**Acceptance criteria:**

- [ ] ADR 1.16 pending line removed.

## Phase 5: The workspace split and the docs

**Components:**

- `crates/harness-sync-core`, `crates/harness-sync-claude-code`, `crates/harness-sync-pi`, `crates/harness-sync-git`,
  `crates/harness-sync` (the binary, with the registry of harnesses and storage kinds, `output`, prompts, the clap CLI).
- Workspace `Cargo.toml` with shared lints, `clippy.toml`, CI and release workflows that find the crates by rule.
- README, `CLAUDE.md`, `.claude/rules/rust.md` (paths of `output.rs` and the e2e tests).

**Testing strategy:**

- Mechanical move: every test passes in its new crate; the end-to-end tests run against the binary crate.

**Verification gate:**

- `cargo tree` shows no dependency between harness crates or between a harness crate and a storage crate.

**Acceptance criteria:**

- [ ] ADR 1.15 pending line removed.
- [ ] README documents Claude Code and pi, the harness commands, and path ids.

## Phase Sequence

Phase 1 (tracer bullet, no deps)
    ↓
Phase 2 (depends on Phase 1)
    ↓
Phase 3 (depends on Phase 2: a pi machine needs a harness list)
Phase 4 (depends on Phase 1; parallel with Phases 2 and 3, but shares `push.rs` and `session.rs` with them)
    ↓
Phase 5 (depends on Phases 3 and 4)

One agent runs the phases in order, because Phases 2 to 4 edit the same command modules.

## Scope Boundaries

**In scope:** the Claude Code and pi harnesses, path ids and the home-mirror layout, harness lists per machine, root
files, the storage trait with the git and Gerrit kinds, the crate split, the docs.

**Out of scope:** Codex, kensai-local, TOML and YAML editors, storage kinds other than git and Gerrit, a stable core API
for outside crates, a migration of repos in the earlier layout, the regeneration of the existing sync repo (it needs a
force-push, which waits for the user).
