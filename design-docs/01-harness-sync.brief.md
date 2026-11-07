# Brief: harness-sync

## Motivation

The user-level setup of agent harnesses moves to other machines by hand: when a machine is set up, and each time a
global setting changes. The user runs Claude Code and pi; Codex and kensai-local come later. Each harness keeps its
setup in its own directory under the home directory, mixed with state that the harness writes for itself. As prior art,
a colleague keeps a hand-maintained git repo with one folder per harness, folders for tools outside the harnesses, and
an instruction file at the root.

## Goals

- Set up a new machine from the remote snapshot with one pull.
- Move a change from any machine to the others: push on the machine with the change, then pull on each other machine.
- Sync each harness on its own. A value of one harness never becomes a value of another harness.
- The first release syncs Claude Code and pi. Codex and kensai-local follow in later releases.
- Each machine's sync configuration lists the harnesses that the machine syncs. A machine with Claude Code and without
  pi gets only Claude Code.
- The repo holds one folder per harness, named after the harness directory under the home directory (`.claude`, `.pi`),
  and root files that belong to no harness, such as a plain instruction file that several harnesses reuse. Root files
  are tracked relative to the home directory.
- Store the remote snapshot in a git repo on any git remote, or in a Gerrit project. The storage sits behind a seam, so
  that other kinds of storage can come later.
- Build the code as a core crate, one crate per harness, and storage crates, compiled into one binary. Third parties
  contribute crates, or build their own binary with their own crates.
- Before push and before pull, list the items that differ from the remote snapshot.
- Sync the plugin setup of each harness.
- Partial sync: one machine can keep part of the setup out of the sync. Example: a test machine that enables something
  the remote snapshot disables.
- Never put a secret in the repo as plain text. Masking is the minimum; the user enters the real value on each machine.
- Run on Linux, Windows, and macOS.
- Publish as open source. Each user has their own repo.

## Non-goals

- Interop between harnesses: moving or translating a setting from one harness to another. A shared file at the root is a
  plain file; how a harness uses it is the user's setup.
- Automatic sync (watchers, hooks, schedules).
- A master machine. All machines are equal.
- Auto-memory and the data of third-party plugins, such as claude-mem.
- Project-level setup.
- Plugins of harness-sync loaded at run time.

## Constraints

- The storage of the first release is a git repo, plain or on Gerrit.
- Push and pull are manual commands.
- Linux, Windows, and macOS.
- No plain-text secret in the repo.
- Nothing is released. Formats, names, and the design can change without a migration path.
- The public docs never name company-internal structure: hosts, repos, people, or the folders of internal tools.
  kensai-local may be named.

## Key risks

- Secret leak. Git history keeps a pushed secret even after a later commit removes it, and users can push to a public
  repo. Masking depends on finding every secret, and each harness brings its own key names.
- Lost changes. There is no master, so two machines can change the same item between syncs.
- Wrong values on another OS. Hook commands, absolute paths, and status line commands can break when they move from one
  OS to another.
- Partial sync leak. An item that one machine keeps out can still go out in a push.
- Item lists go stale. A harness release moves or changes a configuration file.
- A core shaped by one harness. Claude Code, pi, Codex, and kensai-local use different files and different formats.

## Flagged term ambiguities

- "harness": any program with a user-level setup that harness-sync syncs, including kensai-local, which is a code-review
  CLI and not a coding agent. A new glossary entry.
- "target": the glossary defines it as a named git repo and branch. Storage that is not git changes the definition, and
  the kind of storage needs its own term.
- "sync configuration": it also lists the harnesses of the machine.
- "root setup": the files at the repo root that belong to no harness.

## Questions for research

- Which files make up the user-level setup of Claude Code, pi, Codex, and kensai-local? Which of them sync between
  machines, which does the harness write at run time, and which hold credentials?
- Which values hold secrets in each harness, and how can a tool find them?
- Which values depend on the OS or on the machine?
- How does each harness store its plugin setup, and can another machine rebuild it from configuration alone?
- Which existing tools sync the setup of one or several harnesses between machines, and how do they handle secrets,
  per-machine values, and conflicts?
- Which parts of the existing code name a harness, and which depend on git?

## Not yet specified

- The form of an item id that names its harness.
- One `harness-sync.toml` at the root, or one per harness folder.
- Whether git and Gerrit are separate crates.
- Whether the core crate is published with a stable API for crates outside this repo.
