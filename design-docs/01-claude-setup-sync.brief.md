# Brief: Claude Code user-level setup sync

## Motivation

Today the user-level Claude Code setup moves to other machines by hand. This happens when a new machine is set up and
every time a global setting changes. `claude-sync` was not tried: its feature list did not show git repo storage, memory
sync, or plugin setup.

## Goals

- Set up a new machine from the remote snapshot with one sync command.
- Move a change from any machine to the others: push on the machine with the change, then sync on each other machine.
- Store the remote snapshot in a git repo on any git remote.
- Before push and before sync, list the supported items that differ from the remote snapshot.
- Include the plugin setup as a supported item.
- Partial sync: a sync configuration lets one machine take only part of the remote snapshot. Example: a test
  environment that enables something the remote snapshot disables.
- Never put a secret in the repo as plain text. Masking is the minimum; the user enters the real value on the target
  machine.
- Run on Linux, Windows, and macOS.
- Publish as open source. Each user has their own repo.

## Non-goals

- Automatic sync (watchers, hooks, schedules).
- A master machine. All machines are equal.
- Auto-memory (`~/.claude/projects/<project>/memory/`): it is project-specific, and the projects on each machine differ.
- claude-mem data: it belongs to a third-party plugin.
- Project-level setup.

## Constraints

- The storage is a git repo.
- Push and sync are manual commands.
- Linux, Windows, and macOS.
- No plain-text secrets in the repo.

## Key risks

- Secret leak. Git history keeps a pushed secret even after a later commit removes it. Open-source users can push to a
  public repo. Masking depends on the tool finding every secret.
- Lost changes. There is no master, so two machines can change the same item between syncs. A push can drop the other
  machine's change.
- Wrong values on another OS. Hook commands, absolute paths, and the status line command can break when they are synced
  from one OS to another.
- Partial sync leak. If an excluded item can still go out in a push, a test-only change reaches every machine. Partial
  sync was called "simple to do", but its granularity is not known.
- Item list goes stale. A Claude Code release can move or change a configuration file.

## Flagged term ambiguities

- "memories": auto-memory and claude-mem are both excluded, but memory sync was one of the three reasons named against
  `claude-sync`. The Claude Code documentation calls the user-level `CLAUDE.md` "user memory". If that is the meaning,
  it is a supported item.
- "sync": the word names the whole function of the tool and also the pull command. One name for two concepts.
- "global settings" / "user-level setup": two names for one concept.
- New concepts: supported item, remote snapshot, sync configuration (partial sync).

## Questions for research

- Which `claude-sync` is it (several projects use the name)? Do it or similar tools already have git storage, plugin
  setup sync, partial sync, or secret masking?
- Which files and state make up a Claude Code user-level setup on Linux, Windows, and macOS? Which of them can sync
  between machines?
- Which values hold secrets, and how can a tool find them?
- Which values depend on the OS or the machine?
- How does Claude Code store the plugin setup (marketplaces, installed plugins, enable flags)? Can another machine
  rebuild the plugin setup from configuration alone?
- What does Claude Code already provide that covers part of this (settings layers, managed settings, git-based plugin
  marketplaces)? Can a local layer override a synced value?

## Not yet specified

- Item granularity: a whole file or one key.
- What a machine on another OS gets for OS-specific values.
- How a listed item is resolved when two machines changed it.
- How the user enters a masked secret, and what the tool does while the secret is missing.
- Partial sync: what the configuration selects, where it is stored, and whether excluded items are kept out of push.
- Language: Node or Go.
- Tool name.
