# harness-sync

harness-sync syncs the user-level setup of the Claude Code and pi harnesses between machines through a git
repo. Each machine pushes its own changes and pulls the changes of the others; no machine is the master. The design is
in `design-docs/`.

## Vocabulary

- Name every concept with its term from `docs/glossary.md`. Never use a name that an **Avoid** line of the glossary
  lists.

<public-text>

- Never name company-internal structure in docs, ADRs, the README, or commit messages: no internal host, no internal
  repo name, no person's name, no folder layout of an internal tool.
- Describe internal prior art by its shape only.
- kensai-local may be named.

</public-text>

@design-docs/ADR.md
