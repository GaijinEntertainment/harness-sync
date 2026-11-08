# harness-sync

harness-sync syncs the user-level setup of the Claude Code and pi harnesses between machines through a git
repo. Each machine pushes its own changes and pulls the changes of the others; no machine is the master. The design is
in `design-docs/`.

## Vocabulary

- Name every concept with its term from `docs/glossary.md`. Never use a name that an **Avoid** line of the glossary
  lists.

@design-docs/ADR.md
