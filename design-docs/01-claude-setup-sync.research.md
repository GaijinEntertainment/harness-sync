# Research: Claude Code user-level setup sync

- **Date:** 2026-11-04
- **Teammates:** 3
- **Scopes investigated:** local install (one macOS machine), Claude Code documentation and changelog, existing tools
  (Claude Code configuration sync tools, chezmoi, yadm)

Values that identify the machine owner are generalized: `~` is the home directory, and org UUIDs, account UUIDs,
internal host names, MCP server names, and private marketplace names are left out.

## Local install

### Files investigated

- `~/.claude/` - top two levels; transcripts (`*.jsonl`) and `plugins/cache/` contents not read
- `~/.claude/settings.json`, `~/.claude/settings.json.bak`, `~/.claude/keybindings.json`
- `~/.claude.json` - 5185 lines, 105 top-level keys; `~/.claude/.claude.json` - first 30 lines
- `~/.claude/plugins/{installed_plugins,known_marketplaces,blocklist,flagged-plugins}.json`, `~/.claude/plugins/synced/`
- `~/.claude/statusline.mts`, `~/.claude/hooks/rtk-rewrite.sh`
- `/Library/Application Support/ClaudeCode` - does not exist

### Findings

#### Top-level content of `~/.claude`

Full name list from `ls -A`:

- Files: `.claude.json`, `.last-cleanup`, `.last-update-result.json`, `CLAUDE.md`, `RTK.md`, `daemon-auth-cooldown`,
  `daemon-auth-status.json`, `daemon.log`, `history.jsonl`, `keybindings.json`, a stray `*.patch` file,
  `policy-limits.json` (with `.signature.json`, `.signature-iat.json`, `.stamp.json`), `remote-settings.json` (with
  `.signature.json`, `.signature-iat.json`, `.synced-at`), `settings.json`, `settings.json.bak`, `stats-cache.json`,
  `statusline.js`, `statusline.mts`.
- Directories: `backups/` (`.claude.json.backup.<epochMs>`), `cache/` (`changelog.md`, `my-closed-issues.json`),
  `daemon/` (`roster.json`, `control.key`), `debrief-research/`, `debug/`, `feedback/`, `file-history/`, `guides/`
  (`notebooklm-research.md`, linked from `CLAUDE.md` by relative path), `hooks/` (`rtk-rewrite.sh`,
  `.rtk-hook.sha256`), `ide/` (`<port>.lock`), `jobs/` (`pins.json`), `paste-cache/` (25 `<16hex>.txt`), `plugins/`,
  `projects/`, `session-env/`, `sessions/` (`<pid>.json`, `<pid>.<64hex>.key`), `shell-snapshots/`
  (`snapshot-zsh-<epochMs>-<id>.sh`), `skills/`, `state/` (`mcp-discover-verdicts.json`), `tasks/`
  (`session-<8hex>/`), `teams/` (29 dirs with `config.json`), `telemetry/` (48 `1p_failed_events.*.json`), `uploads/`.
- Not present: `rules/`, `agents/`, `commands/`, `output-styles/`, `todos/`, `statsig/`, `plans/`.
- `projects/<slug>/`: slug is the absolute working directory with `/` and `.` replaced by `-`. Holds
  `<sessionUuid>.jsonl`, `<sessionUuid>/subagents/agent-<id>.jsonl` and `.meta.json`, and `memory/` (17 projects have
  `memory/MEMORY.md`).
- `skills/`: 6 user skills as `skills/<name>/SKILL.md` (one has `references/`), plus `skills/synced/<bucket>/<skill>/`
  in 4 buckets named `<orgUuid>_<accountUuid>`.

The teammate's Glob-based listing reported `file-history/` and `history.jsonl` as absent; the direct `ls -A` shows both.

#### `settings.json` structure

- `env` (object of strings): LSP, agent-teams, flicker, and telemetry flags; OTLP exporter protocol, endpoints, and
  `OTEL_EXPORTER_OTLP_HEADERS`.
- `attribution` (`commit`, `pr`), `permissions` (`deny`: 13 tool names, `defaultMode`).
- 12 booleans: `disableClaudeAiConnectors`, `disableBundledSkills`, `disableWorkflows`, `disableArtifact`,
  `workflowKeywordTriggerEnabled`, `skipDangerousModePermissionPrompt`, `autoCompactEnabled`,
  `precomputeCompactionEnabled`, `idleCompaction`, `switchModelsOnFlag`, `remoteControlAtStartup`,
  `agentPushNotifEnabled`.
- `skillOverrides` (`<plugin>:<skill>` -> `"off"`, 30 entries), `statusLine` (`type`, `command`, `refreshInterval`),
  `enabledPlugins` (`<plugin>@<marketplace>` -> bool, 9 entries, including `@synced` and `@builtin` ids),
  `extraKnownMarketplaces` (`<name>` -> `{source: {source, repo}, autoUpdate?}`, 2 entries), `effortLevel`,
  `modelSettings` (`<modelId>` -> `{effortLevel, autoCompactWindow?}`), `pluginConfigs` (`<plugin>@<marketplace>` ->
  `{options}`, 2 entries).
- No `hooks`, `mcpServers`, `apiKeyHelper`, or `awsAuthRefresh` key.
- `settings.json.bak` holds an older key set (`env`, `permissions`, `statusLine`, `enabledPlugins`, `effortLevel`,
  `voiceEnabled`, `autoDreamEnabled`, `skipDangerousModePermissionPrompt`).
- `keybindings.json`: `$schema`, `$docs`, `bindings` (array of `{context, bindings: {key: action}}`, 19 contexts).

#### `~/.claude.json` structure

- Counters (about 25 keys), for example `numStartups`, `tipsHistory`, `skillUsage`, `pluginUsage`.
- Timestamps, for example `firstStartTime`, `changelogLastFetched`, `cachedGrowthBookFeaturesAt`.
- Caches, for example `cachedGrowthBookFeatures` (lines 66-1625), `clientDataCacheSlots`, `modelAccessCache`.
- Account and identity: `userID`, `machineID`, `oauthAccount` (account and org UUIDs, email, names, roles, tiers),
  `customApiKeyResponses`, `claudeAiMcpEverConnected`, referral data in `passesEligibilityCache`.
- Feature and migration flags: `migrationVersion`, many `hasSeen*` / `hasCompleted*` booleans, `lastOnboardingVersion`.
- Configuration: `mcpServers`, `githubRepoPaths` (repo slug -> absolute paths), `installMethod`, `autoUpdates`,
  `companion`, and `autoCompactEnabled`, `remoteControlAtStartup`, `agentPushNotifEnabled` (these three keys also exist
  in `settings.json`).
- `projects`: 27 entries keyed by absolute path. Record fields: `allowedTools`, `mcpContextUris`, `mcpServers`,
  `enabledMcpjsonServers`, `disabledMcpjsonServers`, `disabledMcpServers`, trust flags (`hasTrustDialogAccepted`,
  `hasClaudeMdExternalIncludes*`), last-session metrics (`lastCost`, `lastDuration`, token counts, `lastModelUsage`,
  `lastSessionMetrics`), `lastSessionId`, `lastSessionFirstPrompt` (raw prompt text), `exampleFiles`,
  `activeWorktreeSession` (absolute paths, branch, commit). Not every record has every field.
- Key order is not alphabetical; cache keys are interleaved with configuration keys.
- `~/.claude/.claude.json` is a second, smaller file with `cachedGrowthBookFeatures`. `backups/.claude.json.backup.*`
  are copies of `~/.claude.json`.

#### Plugin and marketplace records

- `plugins/installed_plugins.json`: `{version: 2, plugins: {"<plugin>@<marketplace>": [record, ...]}}`, 26 plugin ids.
  Record fields: `scope` (`user` | `project` | `local`), `projectPath` (absent for `user`), `installPath` (absolute,
  `plugins/cache/<marketplace>/<plugin>/<version>`), `version`, `installedAt`, `lastUpdated`, `gitCommitSha`, `auto`.
  One plugin can have dozens of records, one per project path or worktree. No source, URL, or ref field.
- `plugins/known_marketplaces.json`: `<name>` -> `{source: {source: "github", repo}, installLocation (absolute),
  lastUpdated, autoUpdate?}`, 5 entries, all of source type `github`. No ref or commit field.
- `plugins/blocklist.json`, `plugins/flagged-plugins.json`, `plugins/.last_inuse_sweep`, two large catalog caches
  (`plugin-catalog-cache.json`, `plugin-directory-cache-v2.json`; not read).
- `plugins/synced/<bucket>/`: `<plugin>.meta.json` (`server_plugin_id`, `marketplace_name`,
  `installation_preference`), `.marketplaces.json` (source `claudeai`), `.last-complete-round`.

#### MCP server definitions

- `~/.claude.json` top-level `mcpServers`: 7 servers. 2 `http` with an inline `headers.Authorization`, 1 `http` to a
  localhost IDE port, 4 `stdio` with a bare command name resolved through PATH (one runs `uvx` with a `git+https` URL,
  one has an `env` entry).
- `~/.claude.json` per-project `mcpServers`: 26 of 27 are empty; 1 holds an `http` server.
- Plugin-shipped `.mcp.json` at the plugin root (`{"mcpServers": {...}}`), found in 7 plugin directories.
- Definition shape: stdio = `{type, command, args, env?}`; http = `{type, url, headers?}`.

#### Values tied to the machine

- Absolute paths under `~`: `statusLine.command` (`node "<~>/.claude/statusline.mts"`), `installed_plugins.json`
  `installPath` and `projectPath`, `known_marketplaces.json` `installLocation`, `~/.claude.json` `projects` keys,
  `activeWorktreeSession.*`, `githubRepoPaths`, `sessions/<pid>.json` `cwd`, `ide/<port>.lock` `workspaceFolders`,
  `teams/*/config.json` `members[].cwd`.
- Host names: OTLP endpoints in `settings.json` `env`; MCP server URLs in `~/.claude.json`.
- Commands: `statusLine.command`; `mcpServers.*.command` + `args`.
- Identity: `oauthAccount`, `userID`, `machineID`.

#### Credential-bearing keys (key paths only)

- `settings.json` and `settings.json.bak`: `env.OTEL_EXPORTER_OTLP_HEADERS` (bearer header; same key in both files).
- `~/.claude.json`: `mcpServers.<server>.headers.Authorization` (2 servers), `customApiKeyResponses.approved[]`,
  referral codes in `passesEligibilityCache`. No OAuth access or refresh token key.
- `ide/<port>.lock`: `authToken`. `sessions/*.key`, `daemon/control.key`: content not read.
- No `.credentials.json`; on macOS the login token is in the Keychain (see documentation findings).

#### Hooks and executables

- No hook is registered in user settings. `hooks/rtk-rewrite.sh` exists and needs `jq` and an external `rtk` binary; no
  file read registers it.
- `statusLine` runs `node` on `statusline.mts` inside `~/.claude`.
- Plugin hooks live inside plugin directories.

### Patterns

- The `<plugin>@<marketplace>` id keys `installed_plugins.json`, `enabledPlugins`, `pluginConfigs`, and
  `flagged-plugins.json`; `skillOverrides` uses `<plugin>:<skill>`.
- Absolute home paths appear in 10 locations listed above, in 5 files and 3 runtime directories.
- One secret value exists in 2 files (`settings.json`, `settings.json.bak`).

### Integration points

- `statusLine.command` -> `statusline.mts` -> `node` on PATH.
- `mcpServers.*.command` -> executables on PATH outside `~/.claude`.
- `installed_plugins.json` `projectPath` -> `~/.claude.json` `projects` keys.

### Open gaps

- `stats-cache.json`, `telemetry/`, `paste-cache/`, `shell-snapshots/`, and `backups/` contents not read.
- The purpose of `~/.claude/.claude.json` is not determined.

## Claude Code documentation and changelog

### Files investigated

Raw Markdown from `https://code.claude.com/docs/en/`: `settings.md`, `settings-reference.md`, `claude-directory.md`,
`env-vars.md`, `memory.md`, `mcp.md`, `managed-mcp.md`, `managed-settings.md`, `server-managed-settings.md`,
`authentication.md`, `setup.md`, `hooks.md`, `statusline.md`, `skills.md`, `commands.md`, `cloud-environments.md`,
`plugins/{marketplace-reference,cli-reference,loading,install,org,manifest-reference,components}.md`; changelog
`https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md` (newest entry 2.1.318).

### Findings

#### Settings scopes, paths, precedence

Sources: `settings.md`, `managed-settings.md`.

- User: `~/.claude/settings.json`; Windows `%USERPROFILE%\.claude\settings.json`; moves under `CLAUDE_CONFIG_DIR`.
- Project: `.claude/settings.json`. Local: `.claude/settings.local.json` (project-level; since 2.1.211 kept at the git
  repo root, except on Windows and some other cases).
- Managed: macOS `/Library/Application Support/ClaudeCode/managed-settings.json`, Linux and WSL
  `/etc/claude-code/managed-settings.json`, Windows `C:\Program Files\ClaudeCode\managed-settings.json`; drop-ins in
  `managed-settings.d/*.json`; macOS plist domain `com.anthropic.claudecode`; Windows registry
  `HKLM`/`HKCU\SOFTWARE\Policies\ClaudeCode`; server-managed settings cached in `~/.claude/remote-settings.json`.
- Command line: `--settings <file-or-json>`, `--setting-sources user,project,local`.
- `~/.claude.json` is a separate "global config" file written by Claude Code; a fixed list of global-config keys applies
  only from it.
- Precedence, highest first: managed, `--settings`, local, project, user. Environment variables are not a level; some
  variables beat a specific key.
- The documented scopes have no second user-level file. `settings.local.json` is project-level only.
- Merge: list values (for example `permissions.allow`) merge across scopes. Whole-value exceptions: `fallbackModel`,
  `modelPicker`, `availableModels` (managed), `modelSettings` (per model). An `extraKnownMarketplaces` entry of one name
  from the higher file replaces the lower one whole (2.1.228+). `enabledPlugins` merges key by key.
- Claude Code watches settings files and reloads most keys live. A broken `~/.claude.json` is copied to
  `~/.claude/backups/.claude.json.corrupted.<timestamp>`; the five newest `.claude.json.backup.<timestamp>` are kept.

#### Contents of the user configuration directory

Sources: `claude-directory.md`, `env-vars.md`, `memory.md`.

- Authored: `settings.json`, `CLAUDE.md`, `rules/*.md`, `skills/<name>/SKILL.md`, `commands/*.md`, `output-styles/*.md`,
  `agents/*.md`, `workflows/*.js`, `agent-memory/<name>/`, `themes/*.json`, `keybindings.json`.
- Hooks and the status line exist only as keys in settings files (and in plugin, skill, and agent definitions); a
  status line command can point to a script file.
- Plugins: `plugins/{marketplaces,cache,data/<plugin-id>,synced,.trash}/`, `installed_plugins.json`,
  `known_marketplaces.json`, `known_marketplaces_claudeai.json`, `flagged-plugins.json`. `skills/synced/` holds
  claude.ai-synced skills.
- `projects/<project>/memory/` is auto memory: "machine-local", "not shared across machines". Relocatable with
  `autoMemoryDirectory`.
- Written by Claude Code: `history.jsonl`, `stats-cache.json`, `remote-settings.json`, `policy-limits.json`, `cache/`,
  `plans/`, `debug/`, `paste-cache/`, `file-history/`, `tasks/`, `shell-snapshots/`, `session-env/`, `sessions/`,
  `backups/`, `usage-data/`, `jobs/`, `daemon/`, `dev-mods/`, `uploads/`, `.credentials.json`. Legacy `todos/`,
  `statsig/`, `logs/` are no longer written. `cleanupPeriodDays` (default 30) sweeps them.
- `~/.claude.json`: app state, account data, UI toggles, per-project state, user-scope and local-scope MCP servers.
  With `CLAUDE_CONFIG_DIR` set, it lives under that directory.
- Relocation variables: `CLAUDE_CONFIG_DIR` (all settings, history, plugins, credentials file, Keychain entry key),
  `CLAUDE_CODE_PLUGIN_CACHE_DIR`, `CLAUDE_CODE_PLUGIN_SEED_DIR` (read-only seed, several dirs),
  `CLAUDE_CODE_PLUGIN_DIRS`, `CLAUDE_CODE_PROJECT_DIR_NAME`, `CLAUDE_CODE_TMPDIR`. Claude Code does not use XDG
  directories for its own files.
- `CLAUDE.md` `@path` imports accept relative, absolute, and `@~/` paths; depth limit 4.

#### Plugins and marketplaces

Sources: `plugins/*.md`, `settings-reference.md`.

- Marketplace source types: `url`, `github` (`repo`, `ref`, `path`, `sparsePaths`), `git` (any host), `file`,
  `directory`, `settings` (inline), `npm` (not implemented for `extraKnownMarketplaces`).
- `enabledPlugins`: six sources; the highest source that names the id wins; managed `false` blocks install.
- `extraKnownMarketplaces` entries from repository files apply only after the workspace trust dialog.
- `pluginConfigs` is read from user, `--settings`, or managed only (2.1.207+). Sensitive plugin options are stored in
  the Keychain or `.credentials.json`.
- Startup: plugins load from `installed_plugins.json` and the cache with no network. Then, in the background, a
  marketplace declared in settings but absent from `known_marketplaces.json` is cloned, and enabled plugins that are
  not cached are downloaded; `/reload-plugins` activates them.
- A plugin enabled only in project `.claude/settings.json` is not fetched on a machine that does not have it. External
  sources are fetched only when `true` comes from user settings, an untracked `settings.local.json`, `--settings`, or
  managed settings.
- CLI: `claude plugin marketplace add <source> [--scope]`, `marketplace list|update|remove`,
  `claude plugin install <plugin> [-s scope] [-y] [--marketplace <source>]`, `uninstall`, `enable`, `disable`,
  `update`, `list [--json]`. Without a TTY and without `-y`, installs that run a command or a `headersHelper` are
  refused. A fresh non-interactive machine has no official marketplace registered.

#### MCP servers

Sources: `mcp.md`, `managed-mcp.md`.

- Scopes: local (`~/.claude.json` `projects["<path>"].mcpServers`, default), project (`.mcp.json`), user
  (`~/.claude.json` top-level `mcpServers`). `claude mcp add --scope user|project|local`, `claude mcp add-json`.
- Same name in several scopes: the whole entry from the highest source (local > project > user > plugin > connectors).
- Managed: `managed-mcp.json` (exclusive) or the `managedMcpServers` key.
- Secrets: `${VAR}` and `${VAR:-default}` expand in `command`, `args`, `env`, `url`, `headers`. Expansion applies to
  project `.mcp.json` and to user-scope and local-scope entries in `~/.claude.json` (`mcp.md` lines 121, 881). An unset
  variable with no default loads with the literal text and a warning in `claude mcp list`. In a remote server's `url`
  and `headers`, a fixed set of credential variables (for example `ANTHROPIC_API_KEY`) read as empty. `/mcp` and
  `claude mcp list` show `${VAR}` by name.
- `headersHelper`: a command that prints a JSON header object; 10 s timeout; re-run on 401/403.
- OAuth for remote servers: `/mcp` or `claude mcp login`; tokens in the macOS Keychain (changelog 2.1.118, 2.1.281);
  the non-macOS storage path is not stated.
- Import: `claude mcp add-from-claude-desktop` (macOS and WSL), `/import codex|gemini|cursor`.

#### Variable and path expansion

Sources: `hooks.md`, `statusline.md`, `env-vars.md`, `plugins/manifest-reference.md`.

- Settings `env` block: values are copied as written; `~` and `$HOME` are not expanded.
- Hooks: `${CLAUDE_PROJECT_DIR}`, `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}` are substituted in `command` and
  `args` and exported. Exec form (with `args`): no shell, no `~` or `$HOME` expansion. Shell form: `sh -c` on
  macOS/Linux, Git Bash on Windows, PowerShell when Git Bash is absent.
- Status line: the command runs in a shell; `~` works, also on Windows; no placeholder substitution is documented.
- `autoMemoryDirectory`, plugin directory variables, and `marketplace add` accept `~/` paths.

#### Windows

Sources: `setup.md`, `hooks.md`, `statusline.md`, `settings-reference.md`.

- `~/.claude` = `%USERPROFILE%\.claude`; `~/.claude.json` = `%USERPROFILE%\.claude.json`; credentials in
  `%USERPROFILE%\.claude\.credentials.json`.
- Git for Windows is optional. With it, the Bash tool, shell-form hooks, and the status line use Git Bash; without it,
  PowerShell. Per-hook `shell: "bash" | "powershell"`. A bash-written hook without `shell` runs in PowerShell when Git
  Bash is absent.
- Exec-form hooks cannot start `.cmd`/`.bat` shims such as `npx`.
- Status line under Git Bash: backslashes are removed unless quoted; forward slashes work.
- `apiKeyHelper` runs through `cmd` on Windows, `/bin/sh` elsewhere.

#### Credentials

Sources: `authentication.md`.

- macOS: Keychain; falls back to `~/.claude/.credentials.json` (mode 0600) when the Keychain rejects the write.
- Linux: `~/.claude/.credentials.json` (0600). Windows: `%USERPROFILE%\.claude\.credentials.json`.
- With `CLAUDE_CONFIG_DIR`, the file and the Keychain entry are keyed to that directory.
- Also: `apiKeyHelper`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`.

#### Export, import, and delivery features

Sources: `claude-directory.md`, `plugins/loading.md`, `skills.md`, `server-managed-settings.md`,
`cloud-environments.md`.

- No documented feature exports or syncs `~/.claude` between devices.
- Account-bound sync (claude.ai login): account-enabled plugins download to `~/.claude/plugins/synced/` and load as
  `<name>@synced` (2.1.273+); account-enabled skills download to `~/.claude/skills/synced/`; claude.ai connectors
  appear as MCP servers. Off switches: `syncClaudeAiPlugins`, `syncClaudeAiSkills`, `disableClaudeAiConnectors`.
- Project scope is the documented sharing mechanism (commit `.claude/` and `.mcp.json`).
- One-session loading: `--settings`, `--mcp-config`, `--plugin-dir`, `--plugin-url`, `--agents`.
- Managed delivery: server-managed settings (Teams or Enterprise, admin console), MDM, managed files, `policyHelper`.
- Cloud sessions carry over only repository configuration and server-managed settings.

#### Changelog entries that changed user-level configuration

Versions: 2.0.30 to 2.1.318.

- 2.0.62: `attribution` replaces `includeCoAuthoredBy`. 2.0.64: `.claude/rules/`. 2.1.3: commands merged into skills.
- 2.1.18: `~/.claude/keybindings.json`. 2.1.74: `autoMemoryDirectory`. 2.1.78: `${CLAUDE_PLUGIN_DATA}`.
- 2.1.83: sensitive plugin options in the Keychain or the credentials file. 2.1.118: `~/.claude/themes/`.
- 2.1.119: `/config` settings persist to `~/.claude/settings.json` instead of `~/.claude.json`. 2.1.280: stale copies of
  moved preferences can remain in `~/.claude.json`.
- 2.1.207: `pluginConfigs` no longer read from project settings. 2.1.211: local permission rules saved at the repo root.
- 2.1.228: marketplace entries merge as whole entries. 2.1.232: `additionalMarketplaces` and `allowedMarketplaces`
  aliases.
- 2.1.259: `managedMcpServers`; concurrent sessions no longer revert each other's `~/.claude.json` changes.
- 2.1.273: claude.ai plugin and skill sync into `synced/` directories. 2.1.277: `AGENTS.md` support.
- 2.1.283: plugin cache-miss after a moved home or config directory fixed. 2.1.292: `claude plugin install
  --marketplace`.
- 2.1.75: the `C:\ProgramData\ClaudeCode` managed-settings fallback removed.

### Patterns

- Configuration that a user writes and state that Claude Code writes share both `~/.claude/` and `~/.claude.json`.
- Plugin state has two layers: declarations in settings (`enabledPlugins`, `extraKnownMarketplaces`) and install
  records on disk (`installed_plugins.json`, `known_marketplaces.json`, `cache/`).

### Integration points

- User settings declarations -> background marketplace clone and plugin download at session start.
- `${VAR}` in MCP definitions -> the process environment.
- Hooks and the status line -> the platform shell (sh, Git Bash, PowerShell).

### Open gaps

- The changelog has no dates; the 12-month window (2.0.30 onward) is an estimate. Entries without the searched
  keywords can be missing.
- The MCP OAuth token file path on Linux and Windows is not documented.
- `permissions.md` and `self-hosted-environments-configuration.md` were not read in full.

## Existing tools

### Files investigated

READMEs and source files of the repositories below (raw downloads), npm, PyPI, crates.io, and Homebrew API indexes
(`formula.json`, `cask.json`). Stars and dates as of 2026-11-04.

### Findings

#### Projects named `claude-sync`

- `tawanorg/claude-sync` (Go, 316 stars, v1.17.2 2026-10-06; npm `@tawandotorg/claude-sync`).
- `renefichtmueller/claude-sync` (TypeScript, 29 stars, v1.1.0).
- `kneetworks/claude-sync` (TypeScript, 29 stars; sessions only).
- `shaike1/claude-sync` (shell, 35 stars, last commit 2025-07-23).
- `sgkarra/claude-sync` (bash, 0 stars).
- `aoprisan/ccsync` (Rust; crates.io package `claude-sync`, command `ccsync`).
- `shawonashraf/claude-code-sync` (Python; PyPI `claude-code-sync`, command `claude-sync`).
- npm `claude-sync` = `NClin/claude-local-history-sync` (conversation sync).
- PyPI `claudesync` (renamed `ctxsync`, 740 stars) syncs files with claude.ai Projects, not Claude Code configuration.
- Homebrew core has no formula or cask with these names.
- `anthropics/claude-code#36693` ("No native way to sync Claude Code config and skills across machines") was closed
  as `not_planned` on 2026-04-30 for inactivity and locked.

#### Per-tool features (multi-machine configuration tools)

Fields: storage; files; credentials; per-machine and per-OS values; same-file conflict; plugins and MCP; subset; OS.

- `tawanorg/claude-sync` (Go): R2, S3, GCS, WebDAV, no git. Default scope syncs `CLAUDE.md`, `settings.json`,
  `settings.local.json`, `agents`, `commands`, `skills`, `plugins`, `projects`, `plans`, `tasks`, `history.jsonl`,
  `rules`, `workflows`; `~/.claude.json` `mcpServers` only, opt-in. age encryption of everything. `${HOME}` token and a
  `path_map`. Conflict: local kept, remote saved as `<file>.conflict.<ts>`; MCP three-way merge. `paths` and `exclude`.
  macOS, Linux, Windows.
- `renefichtmueller/claude-sync` (TypeScript): git, Gitea, cloud folder, Syncthing, rsync, custom. Include/exclude
  patterns over `~/.claude`; no `~/.claude.json`. Hard denylist (`.credentials.json`, `settings.local.json`, `.env*`,
  keys); no encryption. No value rewriting. Git three-way merge; same lines stop the sync, `--prefer local|remote`.
  Incoming `settings.json` and `plugins/` changes are held until `--accept-incoming`. Per-device include/exclude.
- `FelixIsaac/claude-code-sync` (Go): git. Plain: `CLAUDE.md`, `commands/`, `agents/`, `skills/`, `hooks/hooks.json`,
  `installed_plugins.json`, `known_marketplaces.json`. age-encrypted whole files: `settings.json`,
  `settings.local.json`, `~/.claude.json`, `.credentials.json`. Platform variants by file name (`name.windows.md`,
  `name.unix.md`); plugin paths as `$CLAUDE_DIR`. Conflict: backups and `.local-backup-<ts>`, manual merge. No subset.
  Windows, macOS, Linux.
- `MikeVeerman/jean-claude` (TypeScript, 165 stars): profiles plus optional git. `CLAUDE.md`, `settings.json`,
  `hooks/`, `skills/`, `agents/`, `keybindings.json`, status line script. No secret scan. Copies with overwrite, no
  conflict detection. Plugins linked into profiles, not synced. No subset.
- `Hudint/claudesync` (Go, 0 stars, v0.2.1 2026-11-03): private git repo. `CLAUDE.md`, `keybindings.json`, `skills/`,
  `agents/`, `commands/`, `rules/`, `output-styles/`, `scripts/`, layered `settings.json`, extra `include`. Never:
  `.credentials.json`, `~/.claude.json`, `settings.local.json`, sessions; `localOnlyKeys` (default `env`,
  `apiKeyHelper`, `awsAuthRefresh`, `awsCredentialExport`, `otelHeadersHelper`); secret scan holds back token-like
  content. `settings.json` = shared <- profiles <- `os/<os>.json` <- `hosts/<host>.json`, key by key; `pin <key>`;
  `{{home}}`, `{{claude_dir}}`. Different keys merge; same value: later device wins; git failure: `resolve --prefer`.
  Plugins through `enabledPlugins` and `extraKnownMarketplaces`; MCP not synced. `include`/`exclude`, profiles,
  `move`/`promote`. Windows, macOS, Linux/WSL.
- `LandB/claudesync` (JS + Supabase): hosted or self-hosted Supabase, web dashboard. `CLAUDE.md`, `skills/`, plugin
  manifest, settings. Ignore rules for key files; content not inspected. `{{USER_HOME}}`, `{{CLAUDE_PATH}}`. Dashboard
  conflicts page; semantics not determined. Runs `claude plugin install` for missing plugins. Checkbox selection.
- `zhaojiannet/cclayer` (Go): git repo layers (one base, private overlays) or plain directories; device manifest
  `~/.config/cclayer/device.toml`. Files: `CLAUDE.md`, `rules/`, `skills/`, `output-styles/`, `hooks/`, status line,
  `agents`; selected `settings.json` keys (`settings_keys`); plugin and marketplace lists; MCP as one JSON per server
  with `${VAR}`. `~/.claude.json`, logins, history never read; `permissions.allow`, `permissions.ask`,
  `permissions.defaultMode`, `env` stay on the device; `check` refuses forbidden content. Code-running items need
  confirmation. Conflict: a file edited locally while the layer changed prompts; `push` rebases and stops on conflict.
  Plugin and MCP steps through `claude` CLI (2.1.288+). macOS, Linux, Windows.
- `Cha1000000/claude-code-sync` (Python template repo): git. Sessions, memory facts, `skills/`, `commands/`, `hooks/`,
  `plans/`, MCP from a template, plugin list, merged `settings.json`, host files. MCP secrets as `{{ENV:NAME}}` filled
  from a git-ignored `ccsync-secrets.env`; registered secrets age-encrypted. Path tokens, per-machine project maps,
  scopes per machine or `os:linux`. JSON files three-way merged per key, local wins; other files left for a human.
  Linux, macOS, Windows.
- `aoprisan/ccsync` (Rust): git or age archive; per-machine directories in the repo. `settings.json`, `CLAUDE.md`,
  `keybindings.json`, `rules/`, `skills/`, `commands/`, `agents/`, `agent-memory/`, `output-styles/`, `workflows/`,
  `themes/`, plugin configuration (not checkouts); `~/.claude.json` `mcpServers` only. `.credentials.json` blocked;
  secret-shaped strings abort the snapshot unless `--allow-secrets`. Path remap table, per-machine overlay. Restore
  deep-merges `settings.json` (arrays unioned), backs up first. `restore --only`, include/exclude. Linux, macOS;
  Windows without service.
- `shawonashraf/claude-code-sync` (Python): local directory or git. `skills/`, `hooks/`, `agents/`,
  `keybindings.json`, `CLAUDE.md`, redacted `settings.json`, plugin manifest. Sensitive `env` values set to `""`;
  `status` lists what to re-enter. Never merges; `resolve` picks a version. Restore reinstalls plugins; no MCP.
- `sgkarra/claude-sync` (bash): git. MCP secrets replaced by named placeholders in `mcp.json.template`; pull prompts
  for missing keys; plugin manifest with `setup`.
- `perfectra1n/claude-code-sync` (Rust, 101 stars; sessions first): git or Mercurial. Hard denylist both directions,
  including `~/.claude.json`, so no MCP. `__HOME__`, `__CLAUDE_DIR__`. Three-way against per-artifact bases: both
  changed = keep local and hold it back from push; `pull --interactive`. Linux, macOS, Windows.
- `seilk/claude-teleport` (Claude Code plugin): private GitHub repo, one branch per machine, `main` = union. Secrets
  auto-detected; `settings.local.json`, `.credentials.json` never synced; hooks reviewed. Conflict semantics not
  determined.
- `GiuseppeBellamacina/claude-code-sync` ("setup-sync", plugin): folder or a Claude Artifact. Settings, marketplaces,
  plugins, user-scope MCP (`claude mcp add-json`), skills, agents, commands, output styles, themes, workflows,
  keybindings, `CLAUDE.md`. Secret-named values become `<REDACTED>` and are listed for manual entry. Cross-OS items
  flagged (PowerShell, `C:\` paths, `cmd /c npx`, `.sh`). Pull compares, then `--overwrite`, `--replace`, or `--ask`.
- `davidsmorais/mcpocket`: GitHub repo or gist; MCP `env`/`headers` secrets AES-256-GCM with a passphrase; additive
  pull; also other AI tools.
- `jamie950315/ccsync`: git-crypt encrypted `config/`, host profiles with `settings.host.json`; aborts on token
  signatures.
- `GCShao/cc-sync`: git; blanks secret values on push, keeps local secrets on pull.
- Others: `HawkeyePierce89/CCSync` (macOS app), `btafoya/claude-code-sync` (Ubuntu, `# @machine:<name>` markers),
  `markee2301/claude-code-sync` (rclone crypt to R2), `mariopaglia/claude-config-sync` (Gists), `trananhhh/ccsync`
  (Syncthing), `ChrisOr-Dev/claude-autosync`, `jung-wan-kim/cc-sync-template`, `balingsisi/claude-sync-tool`.
- One-way distribution tools: `HartBrook/staghorn` (source repos into `~/.claude`), `onsails/ccsync` (global and
  project), `x0c/agentsync` (one source to many AI tools), `moghaddas/claude-code-settings-sync` (managed settings).

#### chezmoi and yadm

- chezmoi (MIT, v2.74.0): `.tmpl` templates with per-machine data (`.chezmoi.os`, `.chezmoi.hostname`, `[data]`),
  templated `.chezmoiignore`, `modify_` scripts that rewrite part of an existing file, password-manager template
  functions (17 integrations plus `custom`), encryption with age, gpg, git-crypt, transcrypt. `chezmoi apply` prompts
  when the target changed since chezmoi last wrote it. Docs: `https://www.chezmoi.io/user-guide/`.
- yadm (GPL-3.0): alternate files by `##<condition>` suffix (os, hostname, class, distro, arch), templates, bootstrap
  script, hooks, encryption of listed patterns into one archive (gpg or openssl), git-crypt and transcrypt wrappers.
  Conflicts are git merge conflicts. Docs: `https://yadm.io/docs/`.

### Patterns

- Storage: git is the most common backend; others use object storage, Syncthing, or hosted services.
- `~/.claude.json`: tools that touch it take out only `mcpServers` (`tawanorg`, `aoprisan`, `HawkeyePierce89`) or
  encrypt the whole file (`FelixIsaac`); others exclude it. No tool documents a general per-key merge of it.
- Secrets: four approaches - denylist of files, whole-file encryption, value redaction or placeholders with re-entry,
  and content scan that blocks the push.
- Per-machine values: path tokens (`{{home}}`, `__HOME__`, `${HOME}`) are common; per-OS or per-host layers exist in
  `Hudint/claudesync`, `Cha1000000`, `FelixIsaac` (file variants), `jamie950315`.
- Conflicts: from overwrite with no detection to per-key three-way merge.

### Open gaps

- Conflict semantics of `LandB/claudesync` and `seilk/claude-teleport`.
- Credential handling of `trananhhh/ccsync`.
- Total tool count: GitHub search is capped; more than 50 repositories of this shape exist.

## Cross-cutting observations

- The documentation and the local files agree that `~/.claude.json` mixes MCP configuration with state, caches, and
  identity data, and that `/config` preferences moved from it to `settings.json` (2.1.119) with stale copies left
  behind; three such keys exist in both files on the local machine.
- Plugin setup on a new machine: user settings declarations make Claude Code clone and download (documentation);
  install records hold absolute paths and per-project entries (local files); `Hudint/claudesync`, `cclayer`, and
  `GiuseppeBellamacina` restore plugins through settings keys or the `claude plugin` CLI.
- Secrets observed locally sit in the settings `env` block and in MCP `headers`. The documentation gives `${VAR}`
  expansion for MCP definitions and no expansion for the settings `env` block.
- Machine-specific values observed locally (absolute status line path, PATH-resolved MCP commands) meet documented
  platform differences (sh, Git Bash, PowerShell; no `~` in `env` and exec-form hooks).

## Unresolved questions

- The purpose of `~/.claude/.claude.json`.
- The MCP OAuth token storage path on Linux and Windows.
- The release dates of changelog versions.
