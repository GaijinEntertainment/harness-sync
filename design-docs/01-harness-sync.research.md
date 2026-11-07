# Research: harness-sync

- **Date:** 2026-11-04 to 2026-11-07
- **Teammates:** 11
- **Scopes investigated:** Claude Code (local install, documentation, changelog); pi (shipped documentation, local
  install); Codex (documentation); kensai-local (local install, skill documents); existing tools (sync tools for one or
  several harnesses, rule generators, dotfile managers); the existing harness-sync code

Values that identify the machine owner are generalized: `~` is the home directory, and org UUIDs, account UUIDs,
internal host names, MCP server names, private marketplace names, and credential values are left out.

## Claude Code: local install

### Files Investigated

- `~/.claude/` (top two levels; transcripts and `plugins/cache/` contents not read)
- `~/.claude/settings.json`, `~/.claude/settings.json.bak`, `~/.claude/keybindings.json`
- `~/.claude.json` (5185 lines, 105 top-level keys); `~/.claude/.claude.json` (first 30 lines)
- `~/.claude/plugins/{installed_plugins,known_marketplaces,blocklist,flagged-plugins}.json`, `~/.claude/plugins/synced/`
- `~/.claude/statusline.mts`, one hook script under `~/.claude/hooks/`
- `/Library/Application Support/ClaudeCode` (does not exist)

### Findings

#### Top-level content of `~/.claude`

- Files: `.claude.json`, `.last-cleanup`, `.last-update-result.json`, `CLAUDE.md`, a second Markdown file,
  `daemon-auth-cooldown`, `daemon-auth-status.json`, `daemon.log`, `history.jsonl`, `keybindings.json`, a stray
  `*.patch` file, `policy-limits.json` (with `.signature.json`, `.signature-iat.json`, `.stamp.json`),
  `remote-settings.json` (with `.signature.json`, `.signature-iat.json`, `.synced-at`), `settings.json`,
  `settings.json.bak`, `stats-cache.json`, `statusline.js`, `statusline.mts`.
- Directories: `backups/` (`.claude.json.backup.<epochMs>`), `cache/`, `daemon/` (`roster.json`, `control.key`),
  `debug/`, `feedback/`, `file-history/`, `guides/` (one file, linked from `CLAUDE.md` by relative path), `hooks/`,
  `ide/` (`<port>.lock`), `jobs/`, `paste-cache/`, `plugins/`, `projects/`, `session-env/`, `sessions/` (`<pid>.json`,
  `<pid>.<64hex>.key`), `shell-snapshots/`, `skills/`, `state/`, `tasks/`, `teams/` (`config.json` per team),
  `telemetry/`, `uploads/`.
- Not present: `rules/`, `agents/`, `commands/`, `output-styles/`, `todos/`, `statsig/`, `plans/`.
- `projects/<slug>/`: the slug is the absolute working directory with `/` and `.` replaced by `-`. It holds session
  transcripts, subagent transcripts, and `memory/` (17 projects have `memory/MEMORY.md`).
- `skills/`: 6 user skills as `skills/<name>/SKILL.md`, plus `skills/synced/<bucket>/<skill>/` in 4 buckets named
  `<orgUuid>_<accountUuid>`. Removed skills move to `skills/.trash/<id>/` (found 2026-11-07).

#### `settings.json` structure

- `env` (object of strings): LSP, agent-teams, flicker, and telemetry flags; OTLP exporter protocol, endpoints, and
  `OTEL_EXPORTER_OTLP_HEADERS`.
- `attribution` (`commit`, `pr`), `permissions` (`deny`: 13 tool names, `defaultMode`).
- 12 booleans, for example `disableClaudeAiConnectors`, `autoCompactEnabled`, `remoteControlAtStartup`.
- `skillOverrides` (`<plugin>:<skill>` -> `"off"`), `statusLine` (`type`, `command`, `refreshInterval`),
  `enabledPlugins` (`<plugin>@<marketplace>` -> bool, including `@synced` and `@builtin` ids), `extraKnownMarketplaces`
  (`<name>` -> `{source: {source, repo}, autoUpdate?}`), `effortLevel`, `modelSettings` (`<modelId>` ->
  `{effortLevel, autoCompactWindow?}`), `pluginConfigs` (`<plugin>@<marketplace>` -> `{options}`).
- No `hooks`, `mcpServers`, `apiKeyHelper`, or `awsAuthRefresh` key.
- `keybindings.json`: `$schema`, `$docs`, `bindings` (array of `{context, bindings: {key: action}}`).

#### `~/.claude.json` structure

- Counters (about 25 keys), timestamps, caches (for example `cachedGrowthBookFeatures`, lines 66-1625), account and
  identity (`userID`, `machineID`, `oauthAccount`, `customApiKeyResponses`), feature and migration flags.
- Configuration: `mcpServers`, `githubRepoPaths`, `installMethod`, `autoUpdates`, `companion`, and
  `autoCompactEnabled`, `remoteControlAtStartup`, `agentPushNotifEnabled` (these three keys also exist in
  `settings.json`).
- `projects`: 27 entries keyed by absolute path, with allowed tools, per-project MCP servers, trust flags, last-session
  metrics, and raw prompt text.
- Key order is not alphabetical; cache keys are interleaved with configuration keys.
- `~/.claude/.claude.json` is a second, smaller file with `cachedGrowthBookFeatures`.

#### Plugin and marketplace records

- `plugins/installed_plugins.json`: `{version: 2, plugins: {"<plugin>@<marketplace>": [record, ...]}}`. Record fields:
  `scope`, `projectPath`, `installPath` (absolute), `version`, `installedAt`, `lastUpdated`, `gitCommitSha`, `auto`.
  One plugin can have dozens of records, one per project path. No source, URL, or ref field.
- `plugins/known_marketplaces.json`: `<name>` -> `{source: {source: "github", repo}, installLocation (absolute),
  lastUpdated, autoUpdate?}`. No ref or commit field.
- `plugins/synced/<bucket>/`: claude.ai-synced plugin metadata.

#### MCP server definitions

- `~/.claude.json` top-level `mcpServers`: 7 servers. 2 `http` with an inline `headers.Authorization`, 1 `http` to a
  localhost IDE port, 4 `stdio` with a bare command name resolved through PATH.
- Per-project `mcpServers` in `~/.claude.json`, and plugin-shipped `.mcp.json` files.
- Definition shape: stdio `{type, command, args, env?}`; http `{type, url, headers?}`.

#### Values tied to the machine

- Absolute paths under `~`: `statusLine.command`, install records, `~/.claude.json` `projects` keys,
  `githubRepoPaths`, session and IDE files, team configs.
- Host names: OTLP endpoints in `settings.json` `env`; MCP server URLs.
- Commands: `statusLine.command`; `mcpServers.*.command` and `args`.
- Identity: `oauthAccount`, `userID`, `machineID`.

#### Credential-bearing keys (key paths only)

- `settings.json`: `env.OTEL_EXPORTER_OTLP_HEADERS`.
- `~/.claude.json`: `mcpServers.<server>.headers.Authorization` (2 servers), `customApiKeyResponses.approved[]`.
- `ide/<port>.lock`: `authToken`. On macOS the login token is in the Keychain; no `.credentials.json` exists.

### Patterns

- The `<plugin>@<marketplace>` id keys `installed_plugins.json`, `enabledPlugins`, `pluginConfigs`, and
  `flagged-plugins.json`.
- Absolute home paths appear in 10 places in 5 files and 3 runtime directories.

### Open Gaps

- The purpose of `~/.claude/.claude.json`.

## Claude Code: documentation and changelog

### Files Investigated

Raw Markdown from `https://code.claude.com/docs/en/`: `settings.md`, `settings-reference.md`, `claude-directory.md`,
`env-vars.md`, `memory.md`, `mcp.md`, `managed-mcp.md`, `managed-settings.md`, `server-managed-settings.md`,
`authentication.md`, `setup.md`, `hooks.md`, `statusline.md`, `skills.md`, `commands.md`, `cloud-environments.md`,
`plugins/*.md`; the changelog (newest entry 2.1.318).

### Findings

#### Settings scopes, paths, precedence

- User: `~/.claude/settings.json` (`%USERPROFILE%\.claude\settings.json` on Windows); moves under `CLAUDE_CONFIG_DIR`.
- Project: `.claude/settings.json`; local: `.claude/settings.local.json` (project level only).
- Managed settings in fixed OS paths, MDM, and server-managed settings cached in `~/.claude/remote-settings.json`.
- `~/.claude.json` is a separate "global config" file written by Claude Code; a fixed list of Global config keys applies
  only from it.
- Precedence, highest first: managed, `--settings`, local, project, user. List values merge across scopes; an
  `extraKnownMarketplaces` entry of one name from a higher file replaces the lower one whole (2.1.228+).
- Claude Code watches settings files and reloads most keys live. It keeps the five newest
  `.claude.json.backup.<timestamp>` copies.

#### Contents of the user configuration directory

- Authored: `settings.json`, `CLAUDE.md`, `rules/*.md`, `skills/<name>/SKILL.md`, `commands/*.md`, `output-styles/*.md`,
  `agents/*.md`, `workflows/*.js`, `agent-memory/<name>/`, `themes/*.json`, `keybindings.json`.
- Hooks and the status line exist only as keys in settings files (and in plugin, skill, and agent definitions).
- Plugins: `plugins/{marketplaces,cache,data/<plugin-id>,synced,.trash}/`, `installed_plugins.json`,
  `known_marketplaces.json`. `skills/synced/` holds claude.ai-synced skills.
- `projects/<project>/memory/` is auto memory, "machine-local".
- Written by Claude Code: `history.jsonl`, `stats-cache.json`, `remote-settings.json`, `policy-limits.json`, `cache/`,
  `plans/`, `debug/`, `paste-cache/`, `file-history/`, `tasks/`, `shell-snapshots/`, `session-env/`, `sessions/`,
  `backups/`, `usage-data/`, `jobs/`, `daemon/`, `uploads/`, `.credentials.json`.
- Relocation: `CLAUDE_CONFIG_DIR` moves all settings, history, plugins, the credentials file, and the Keychain entry
  key; `~/.claude.json` then lives under that directory. Claude Code does not use XDG directories.
- `CLAUDE.md` `@path` imports accept relative, absolute, and `@~/` paths.

#### Plugins and marketplaces

- Marketplace source types: `url`, `github`, `git`, `file`, `directory`, `settings`, `npm`.
- Startup: plugins load from the install records and the cache. Then, in the background, a marketplace declared in
  settings and absent from `known_marketplaces.json` is cloned, and enabled plugins that are not cached are downloaded.
- External sources are fetched only when `true` comes from user settings, `settings.local.json`, `--settings`, or
  managed settings.
- CLI: `claude plugin marketplace add|list|update|remove`, `claude plugin install|uninstall|enable|disable|update|list`.

#### MCP servers

- Scopes: local (`~/.claude.json` `projects["<path>"].mcpServers`), project (`.mcp.json`), user (`~/.claude.json`
  top-level `mcpServers`).
- `${VAR}` and `${VAR:-default}` expand in `command`, `args`, `env`, `url`, `headers`.
- OAuth tokens for remote servers are in the macOS Keychain; the non-macOS path is not documented.

#### Variable and path expansion

- The settings `env` block copies values as written; `~` and `$HOME` are not expanded.
- Hooks: exec form has no shell and no `~` expansion; shell form runs `sh -c`, Git Bash, or PowerShell.
- The status line runs in a shell; `~` works.

#### Windows

- `~/.claude` = `%USERPROFILE%\.claude`; `~/.claude.json` = `%USERPROFILE%\.claude.json`.
- Without Git for Windows, shell-form hooks and the status line run in PowerShell. Under Git Bash, backslashes are
  removed unless quoted.

#### Credentials

- macOS: Keychain, with a fallback to `~/.claude/.credentials.json` (0600). Linux and Windows: `.credentials.json`.
- Also: `apiKeyHelper`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`.

#### Export, import, and delivery features

- No documented feature exports or syncs `~/.claude` between devices. Account-bound sync covers only claude.ai plugins,
  skills, and connectors (2.1.273+).

#### Changelog entries that changed user-level configuration

- 2.0.64 `.claude/rules/`; 2.1.18 `keybindings.json`; 2.1.118 `themes/`; 2.1.119 `/config` settings persist to
  `settings.json` instead of `~/.claude.json`; 2.1.280 stale copies of moved preferences can remain in
  `~/.claude.json`; 2.1.228 marketplace entries merge whole; 2.1.273 claude.ai sync into `synced/`; 2.1.277 `AGENTS.md`
  support.

### Patterns

- Configuration that a user writes and state that Claude Code writes share both `~/.claude/` and `~/.claude.json`.
- Plugin state has two layers: declarations in settings, and install records on disk.

### Open Gaps

- The changelog has no dates. The MCP OAuth token path on Linux and Windows is not documented.

## pi

### Files Investigated

- The documentation shipped with pi: `README.md`, `docs/*.md`, `schemas/*.json` of the `@earendil-works/pi-coding-agent`
  package
- `~/.pi/agent/` on this machine, by names and key names only

### Findings

#### Files pi reads

- User level, in the agent directory (`~/.pi/agent` by default; `docs/configuration.md:7-47`):
  - `settings.json`: settings, resource paths, package declarations. Authored, and also written by `/settings`,
    `pi install`, and `pi config`.
  - `keybindings.json`, `models.json` (endpoints, models, overrides): authored.
  - `mcp.json`: MCP servers. Authored, and also written by `pi mcp add|remove` and the `/mcp` screen.
  - `auth.json`: API keys and OAuth credentials, written by `/login` and `/logout`.
  - `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`: context files. Authored.
  - `SYSTEM.md`, `APPEND_SYSTEM.md`: replace or extend the system prompt. Authored.
  - `extensions/`, `skills/`, `prompts/`, `themes/`: authored content.
- Named elsewhere in the docs: `trust.json` (project trust decisions, written by `/trust`), `mcp-auth.json` (MCP OAuth
  tokens), `mcp.log` (rotated at 5 MB), `sessions/` (written at run time).
- pi also reads `~/.agents/skills/`, and at the project level `.pi/` files (after project trust) and `.agents/skills/`.

#### Relocation

- `PI_CODING_AGENT_DIR` moves the agent directory. `PI_CODING_AGENT_SESSION_DIR`, the `sessionDir` setting, and
  `--session-dir` move the sessions. `PI_PACKAGE_DIR` moves the package directory.
- Resource paths in user settings resolve from the agent directory; `~` and absolute paths are allowed.

#### File structure

- `settings.json` is a flat object. The schema also has state-like keys that the settings tables do not list:
  `lastChangelogVersion`, `trackingId`, `deviceId`. `packages`, `extensions`, `skills`, `prompts`, and `themes` are
  arrays. Project settings override user settings; resource lists combine.
- `models.json`: a required `providers` object keyed by provider id, with `apiKey`, `headers`, `baseUrl`, `models[]`,
  and `modelOverrides`. `apiKey` and header values take a literal, `$NAME`/`${NAME}`, or `!command`.
- `keybindings.json`: a flat map from action id (`tui.*`, `app.*`) to a key or a list of keys.
- A theme file is a strict object with `name`, `appearance`, `vars`, `colors`, `export`.

#### Extensions, skills, prompts, themes, and packages

- Each resource kind has a conventional directory, a settings array of extra paths (with `!pattern`, `+path`, `-path`),
  package contents, and a CLI flag.
- `packages` entries are a source string or `{source, autoload, extensions, skills, prompts, themes}`. Sources:
  `npm:<spec>` (installed into the pi npm directory), `git:<host>/<repo>@<ref>` or an https URL (cloned), or a local
  path (loaded in place, relative to the declaring settings file).

#### MCP

- Built in. `~/.pi/agent/mcp.json` holds a top-level `mcpServers` object keyed by server name: `command`, `args`,
  `env`, `cwd` for stdio; `url`, `headers`, `oauth` for HTTP; plus `type`, `timeout`, `enabled`, `exposure`,
  `toolExposure`, `description`. The docs state that Claude Code `mcpServers` entries copy over unchanged.

#### Credentials

- `auth.json` (keyed by provider id), `mcp-auth.json`, and inline values in `models.json` (`apiKey`, `headers`) and
  `mcp.json` (`env`, `headers`, `oauth.clientSecret`).

#### `~/.pi/agent` on this machine

- Files: `settings.json`, `keybindings.json`, `mcp.json` (an empty `mcpServers`), `models.json`, `trust.json`,
  `auth.json` (0600), `models-store.json` (0600, named in no doc that was read), and undocumented caches
  (`codex-usage.json`, `mcp-cache.json`, `mcp-last-seen-version.json`, `mcp-onboarding.json`).
- Directories: `bin/`, `npm/` (a `node_modules` tree), `themes/`, `extensions/` (two `.ts` files), `packages/` (a local
  npm workspace of two packages, each declaring `pi.extensions` in its `package.json`), `sessions/`, and four
  undocumented directories.
- `settings.json` keys: `lastChangelogVersion`, `defaultProvider`, `defaultModel`, `defaultThinkingLevel`, `packages`,
  `terminal`, `compaction`, `images`, `editorPaddingX`, `theme`, `enabledModels`, `extensions`.

### Patterns

- The agent directory mixes authored files, files that commands write, credentials, caches, and sessions.
- Every project-level file mirrors a user-level file, except `auth.json`, `keybindings.json`, and `models.json`.

### Open Gaps

- The purpose of `models-store.json` and of the undocumented files; where git packages are installed.

## Codex

### Files Investigated

Markdown pages of `https://developers.openai.com/codex/` (undated): `config-file/config-basic.md`,
`config-file/config-advanced.md`, `config-file/config-reference.md`, `config-file/environment-variables.md`, `auth.md`,
`agent-configuration/{agents-md,rules,subagents}.md`, `custom-prompts.md`, `hooks.md`, `customization/memories.md`,
`build-skills.md`, `plugins.md`, `extend/mcp.md`; `https://developers.openai.com/plugins/build/plugins.md`. The
`openai/codex` repo's `docs/` holds stubs that link to these pages.

### Findings

#### Files in the user directory (`~/.codex`)

- Authored: `config.toml` (TOML); `<name>.config.toml` (a profile, selected with `--profile`); `AGENTS.md` and
  `AGENTS.override.md`; `hooks.json` (or an inline `[hooks]` table); `rules/*.rules` (Starlark, experimental);
  `prompts/*.md` (custom prompts, deprecated); `agents/*.toml` (subagents); `plugins/<name>/` (personal plugins).
- Written at run time: `auth.json` (plaintext tokens, refreshed), `history.jsonl`, `memories/` or `memories_v2/`,
  `log/`, SQLite state, `packages/standalone`. Codex also writes `rules/default.rules` when the user allow-lists a
  command in the TUI.
- User skills live in `~/.agents/skills`, not in `~/.codex`.

#### Relocation and layering

- `CODEX_HOME` (default `~/.codex`; must exist) moves the user directory. `CODEX_SQLITE_HOME` and `sqlite_home` move the
  SQLite state; `log_dir` moves the logs.
- Layers, highest first: CLI flags and `-c key=value`; project `.codex/config.toml` files from the project root down to
  the working directory (trusted projects only); the profile file; `~/.codex/config.toml`; cloud-managed defaults;
  `/etc/codex/config.toml`; built-in defaults.

#### `config.toml`

- Top-level keys include `model`, `model_provider`, `approval_policy`, `sandbox_mode`, `notify`, `instructions`,
  `model_instructions_file`, `log_dir`, `cli_auth_credentials_store`, `mcp_oauth_credentials_store`.
- Tables include `[mcp_servers.<id>]`, `[model_providers.<id>]`, `[features]`, `[hooks]`, `[agents]`, `[skills]`,
  `[history]`, `[otel]`, `[tui]`, `[marketplaces.<name>]`, `[plugins."<plugin>@<marketplace>"]`, `[permissions.<name>]`,
  and `[projects."<path>"]` with `trust_level`.
- MCP server: stdio `command`, `args`, `env`, `env_vars`, `cwd`; HTTP `url`, `bearer_token_env_var`, `http_headers`,
  `env_http_headers`, `http_headers_helper`, `oauth.client_id`, `oauth.client_secret`; common `enabled`, timeouts, tool
  filters.
- Secrets: provider keys usually appear as the name of an environment variable (`env_key`, `bearer_token_env_var`,
  `env_http_headers`). Literal secret values can appear in `model_providers.<id>.experimental_bearer_token`,
  `model_providers.<id>.http_headers`, `mcp_servers.<id>.http_headers`, `mcp_servers.<id>.env`,
  `mcp_servers.<id>.oauth.client_secret`, `otel.exporter.<id>.headers`, and `shell_environment_policy.set`.
- `[projects."<path>"]` keys are absolute paths of this machine.

#### Credentials

- `~/.codex/auth.json` or the OS credential store, chosen by `cli_auth_credentials_store` (`file`, `keyring`, `auto`,
  `ephemeral`). MCP OAuth credentials follow `mcp_oauth_credentials_store`; their file location is not documented.

#### Instruction files

- Global: `AGENTS.override.md`, else `AGENTS.md`, in `CODEX_HOME`. Project: one file per directory from the project
  root down to the working directory, concatenated. No `@` import syntax is documented.

#### Prompts, skills, plugins, hooks, subagents

- Skills: a directory with `SKILL.md`; user skills in `~/.agents/skills`, project skills in `.agents/skills`.
- Plugins: marketplaces declared in `[marketplaces.<name>]` (git repo or local directory, `ref`, `sparse_paths`) and
  enabled in `[plugins."<plugin>@<marketplace>"]`; installed from `/plugins` or `codex plugin marketplace add`. The
  install cache location is not documented.
- Hooks in `hooks.json` or `[hooks]` run only after the user trusts them by hash.

### Patterns

- One TOML file holds settings, MCP servers, providers, plugin declarations, and per-project trust keyed by absolute
  paths.

### Open Gaps

- The key names inside `auth.json`, the default credential store, and where hook trust records and plugin files are
  stored.

## kensai-local

### Files Investigated

- `~/.kensai/` and `~/.kensai/kensai-local/`, by names and key names only
- The kensai-local and kensai-review skill documents in `~/.claude/skills/`

### Findings

- `~/.kensai/kensai-local/` holds `settings.yml`, `.credentials.yml`, `providers/` (one provider file and a JSON
  schema), `checkouts/`, and `sessions/`. `~/.kensai/` also holds directories that the skill documents do not mention.
- `settings.yml`: `providers.<name>` (`dialect`, `auth`, `subscription`, `streaming`, `request-timeout`, `base-url`);
  `review` (`provider`, `model`, `reasoning-effort`, `max-surfacers`, `index-workers`, `grep-index.storage`,
  `denoise.enabled`); `trackers.<name>` (`type`, `base-url`).
- `.credentials.yml`: `providers.<name>.oauth.{access, refresh, expires, endpoint, account}`,
  `providers.<name>.api-key`, and `trackers.<name>.token`.
- kensai-local reads `settings.yml` and `.credentials.yml`; its TUI writes both. It writes `checkouts/` (git worktrees)
  and `sessions/<id>/` at run time.
- The skill documents name no environment variable or flag that moves the configuration.

### Patterns

- YAML. Credentials sit in a separate file from the settings.

### Open Gaps

- Whether the binary reads a location override.

## Existing tools

### Files Investigated

READMEs, docs, and source files of the repositories below; npm, PyPI, crates.io, and Homebrew indexes. Stars and dates
as of 2026-11-04 (Claude Code tools) and 2026-11-07 (the others).

### Findings

#### Claude Code sync tools

- `tawanorg/claude-sync` (Go): R2, S3, GCS, WebDAV, no git. Syncs most of `~/.claude` and `~/.claude.json`
  `mcpServers`; age encryption of everything; `${HOME}` token; local kept on conflict.
- `renefichtmueller/claude-sync` (TypeScript): git and other backends; include and exclude patterns over `~/.claude`; a
  hard denylist; git three-way merge with `--prefer local|remote`.
- `FelixIsaac/claude-code-sync` (Go): git; age-encrypted `settings.json`, `~/.claude.json`, and `.credentials.json`;
  platform file variants (`name.windows.md`); manual merge.
- `MikeVeerman/jean-claude` (TypeScript): profiles plus optional git; overwrite with no conflict detection.
- `Hudint/claudesync` (Go): private git repo; layered `settings.json` (shared, profiles, OS, host); `pin <key>`;
  `{{home}}`; `localOnlyKeys`; a secret scan; plugins through settings keys; "last writer wins".
- `zhaojiannet/cclayer` (Go): git layers; selected `settings.json` keys; MCP as one JSON per server; plugin and MCP
  steps through the `claude` CLI.
- `Cha1000000/claude-code-sync` (Python): MCP secrets from a git-ignored secrets file; JSON three-way merged per key,
  local wins.
- `aoprisan/ccsync` (Rust): git or age archive; `~/.claude.json` `mcpServers` only; secret-shaped strings abort the
  snapshot; deep merge on restore.
- `perfectra1n/claude-code-sync` (Rust): git or Mercurial; hard denylist including `~/.claude.json`; three-way against
  per-artifact bases.
- `GCShao/cc-sync`: git; blanks secret values on push and keeps local secrets on pull.
- Others: value redaction with re-entry (`shawonashraf/claude-code-sync`, `GiuseppeBellamacina/claude-code-sync`,
  `sgkarra/claude-sync`), git-crypt (`jamie950315/ccsync`), and more than 50 repositories of this shape in total.

#### Sync tools for several harnesses

- `CharlesGuooo/ai-agent-config-sync`: Claude Code, Codex, Cursor, OpenCode, pi; a git repo with a Node installer. It
  manages only the entries it defines, compares three ways, and reports a conflict for a managed entry changed locally.
  One MCP catalog is rendered into each agent's format. Last push 2026-10-20, no release.
- `hjnnjh/coding-agent-config-sync`: a git repo with items listed by repo path and target path; `ignore_fields` per JSON
  item keep the local value; init, pull, push, status, backups. v0.2.0, 2026-02-08.
- `rosinbum/agent-config-sync`: Claude Code only; base and profile folders; symlinks plus `{{ HOME }}` scrubbing.
- `legout/pi-config`: pi only; a sanitized `config/` with `__REQUIRED__` placeholders and a git-ignored `private/`.
- `spxrogers/agentsync` (Go, v0.17.0, 2026-10-13): 32 agents; one canonical source in `~/.agentsync/` projected into
  each agent's format; secrets from an age vault; single-machine, cross-machine left to a dotfile manager.
- `dallay/agentsync` (Rust, v1.53.0): project-level `.agents/` symlinked into each assistant.
- `nicepkg/vsync`, `slash9494/ai-config-sync-manager`, `hatappo/agent-command-sync`: one agent as the source, converted
  into the formats of the others on one machine.

#### One-source rule generators

- `dyoshikawa/rulesync` (v43.7.0, 2026-11-06): 60+ tools including Claude Code, Codex, and pi; `.rulesync/` sources
  generated into each tool's files; a global mode writes user-level files.
- `intellectronica/ruler` (v0.3.46): about 30 agents; concatenates `.ruler/*.md` into each agent's rules file and
  propagates MCP servers.
- `sampleXbro/agentsmesh`, `lbb00/ai-rules-sync`: project-level generation or symlinks.

#### Dotfile managers

- chezmoi (v2.74.0): templates with per-machine data, `.chezmoiignore`, password-manager functions, age/gpg encryption;
  `modify_` scripts and `setValueAtPath` change part of a JSON, TOML, or YAML file; `chezmoi apply` otherwise loses the
  changes of a program that rewrites its own file.
- yadm (3.5.0): a bare git repo with `$HOME` as the work tree; `##<condition>` alternate files; one encrypted archive
  for listed paths; no partial-file merge.
- GNU Stow (2.4.1): a symlink farm; no secrets and no templating; program writes go into the package file.
- dotbot (v1.24.0): YAML-configured symlinks; `if:` conditions per machine; secrets only through plugins.

### Patterns

- Storage: git is the most common backend.
- Secrets: four approaches: a file denylist, whole-file encryption, value placeholders with re-entry, and a content scan
  that blocks the push.
- Per-machine values: path tokens (`{{home}}`, `__HOME__`, `${HOME}`), OS and host layers, or file variants.
- Tools for several harnesses either convert one source into each harness's format, or link files from a repo. Of the
  tools found, `CharlesGuooo/ai-agent-config-sync` alone compares three ways across machines, and it converts one MCP
  catalog into each format.

### Open Gaps

- GitHub search is capped; more tools of each shape exist.

## Existing code

### Files Investigated

- `src/*.rs` (27 modules of one binary crate), `tests/e2e/*.rs`, `Cargo.toml`, `clippy.toml`, `rust-toolchain.toml`,
  `.github/workflows/{ci,release}.yml`

### Findings

#### Item model and catalog

- `src/item.rs`: `enum JsonFile { Settings, Claude }` and `JSON_SOURCES` (`:49-89`); `enum Source { Json(JsonFile),
  File }` (`:91-124`); `struct ItemId { source, path }` and `type Items = BTreeMap<ItemId, Value>` (`:126-146`); a
  `KeyPath` with the same fields (`:148-177`). Text form `settings:/<pointer>`, `claude:/<pointer>`, `file:<path>`
  (`:209-244`). `SPLIT_SETTINGS_KEYS` and the 29 `CLAUDE_KEYS` (`:8-47`).
- `src/setup.rs`: `load_local` reads each JSON source, splits it into items, adds the file items, and drops excluded
  items (`:40-55`); `load_snapshot` reads `settings.json`, `claude.json`, and `files/` from a commit (`:57-82`);
  `json_changes` writes both JSON files on each push (`:91-110`).
- `src/files.rs`: `CATALOG_FILES`, `CATALOG_DIRS`, `SKIPPED_DIRS`, `.credentials.json`; one item per file with
  `{"blob": <id>, "executable": true?}`; snapshot path `files/<path>` (`:15-28,234-244`).
- `src/references.rs`: finds files under the Claude Code directory that JSON strings or Markdown imports and links name
  (`:18-52`).
- `src/paths.rs`: `CLAUDE_CONFIG_DIR` is the only environment variable read; `state_dir` is `~/.harness-sync`
  (`:16-71`).
- `src/declarations.rs`, `src/declare.rs`: `harness-sync.toml` with `secrets`, `include`, and `exclude` lists of key
  paths; `exclude` matches by segment prefix.

#### Write path and value rewriting

- `src/pull.rs`: computes every write first, then writes the JSON sources in `JSON_SOURCES` order and the file items
  last, with one backup per run (`:55-216`).
- `src/json_edit.rs`: edits JSON through the CST of `jsonc_parser`; bytes outside an edited value stay as they were.
- `src/secret.rs`: placeholder `<harness-sync secret KEYPATH>`; name rule on the uppercased key (`AUTHORIZATION`, or
  ends with `_KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `HEADERS`) (`:54-60`); masks `Source::Json` items only (`:71-75`);
  a token scan of 17 prefixes, private keys, and JWTs.
- `src/home.rs`: `<harness-sync home>` replaces the home directory in JSON string values only.
- `src/executables.rs` (`statusLine`, `mcpServers`, `hooks` commands) and `src/marketplaces.rs`
  (`plugins/known_marketplaces.json` against `extraKnownMarketplaces`): Claude Code specific warnings.
- `src/atomic.rs`: write a temporary file in the same directory, copy the permissions, rename.

#### Engine, CLI, and storage

- `src/compare.rs`: `ThreeWay { base, local, remote }` and `ItemState`; imports only `item`.
- `src/session.rs`: loads the config, fetches the target, resolves the remote and base commits, loads the declarations,
  the local items, the local view (masked and tokenized), and the snapshots.
- `src/git.rs`: every repo operation through the `git` executable; refs `refs/remotes/<target>/<branch>`,
  `refs/harness-sync/base`, `refs/harness-sync/pending`; commits through `git fast-import`. No crate imports.
- `src/config.rs`: `Config { primary, pins, targets }` in `config.toml`; `enum Kind { Git, Gerrit }` (`:30-40`).
- The target kind matters only in `push.rs:78-99` and `declare.rs:184-203` (commit message, destination ref, change
  URL, whether the base moves) and `output.rs:122`. `gerrit.rs` holds the three Gerrit functions.
- `src/main.rs`: one clap CLI; every command returns a `Report`; `output.rs` is the only module that writes to the
  terminal.
- Mutual imports: `setup` and `files`; `output` and `pull`.
- Build: `Cargo.toml` with strict `[lints]` (including `missing_docs`, `unreachable_pub`, `print_stdout`); `clippy.toml`
  limits of 4 arguments, 80 lines, nesting 4; toolchain 1.99.1; CI runs fmt, clippy, and tests on three OSes and rumdl
  on Markdown.

#### Tests

- `tests/e2e/support.rs`: a `Machine` is a temporary home directory with a `.gitconfig`; a `Remote` is a bare repo on
  disk; `isolated` sets `HOME`, `USERPROFILE`, `XDG_CONFIG_HOME`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM` and removes
  `CLAUDE_CONFIG_DIR`. The helpers name Claude Code paths (`settings_file()`, `write_settings`).
- 11 e2e files and 44 unit tests in 9 modules. The fixtures are Claude Code files.

### Patterns

- Claude Code names sit in `item.rs`, `paths.rs`, `files.rs`, `references.rs`, `executables.rs`, `marketplaces.rs`, one
  warning in `output.rs`, and one message in `pin.rs`. `compare.rs`, `git.rs`, `gerrit.rs`, `config.rs`, `target.rs`,
  `init.rs`, `atomic.rs`, and `home.rs` name no harness.
- The code edits JSON only, and reads TOML for its own files.

## Cross-Cutting Observations

- **Directories.** Claude Code: `~/.claude/` plus `~/.claude.json` beside it (`CLAUDE_CONFIG_DIR`). pi: `~/.pi/agent/`
  (`PI_CODING_AGENT_DIR`) plus `~/.agents/skills/`. Codex: `~/.codex/` (`CODEX_HOME`) plus `~/.agents/skills/`.
  kensai-local: `~/.kensai/kensai-local/`, no documented override.
- **Formats.** Claude Code JSON; pi JSON, Markdown, and TypeScript extensions; Codex TOML, Markdown, and Starlark rules;
  kensai-local YAML.
- **Shared locations.** pi and Codex both read `~/.agents/skills/`. pi reads `CLAUDE.md` as a context file.
- **MCP servers.** Claude Code `~/.claude.json` `mcpServers` and pi `mcp.json` `mcpServers` share one entry shape; Codex
  uses `[mcp_servers.<id>]` TOML tables with other key names.
- **State in the harness directories.** Every harness writes sessions, caches, credentials, or trust records into its
  user directory. Settings files also hold state: pi `lastChangelogVersion`, Codex `[projects."<path>"]`, Claude Code
  `~/.claude.json` counters and caches.
- **Credentials.** Claude Code `.credentials.json` or the Keychain; pi `auth.json`, `mcp-auth.json`,
  `models-store.json`; Codex `auth.json` or the OS store; kensai-local `.credentials.yml`.
- **Secret names against the existing name rule.** Matched: pi `headers` and `clientSecret`; Codex `http_headers`,
  `client_secret`, `experimental_bearer_token`; kensai `token`. Not matched: pi `apiKey`; kensai `api-key`, `access`,
  `refresh`; Codex `mcp_servers.<id>.env` values with names that end in no secret word.
- **Plugin mechanisms.** Claude Code: `enabledPlugins` and `extraKnownMarketplaces` in settings, install records apart.
  pi: `packages` in `settings.json` (npm, git, or a local path). Codex: `[marketplaces.*]` and `[plugins.*]` in
  `config.toml`.
- **Storage.** All storage access in the existing code goes through `git.rs`; the snapshot is a commit and the base is
  a ref. Only the commit message, the destination ref, the change URL, and the base move differ between git and
  Gerrit.

## Unresolved Questions

- The purpose of `~/.claude/.claude.json` and of pi `models-store.json`.
- The MCP OAuth token storage on Linux and Windows for Claude Code; the MCP OAuth file of Codex.
- Whether kensai-local reads a location override.
- The release dates of Claude Code changelog versions.
