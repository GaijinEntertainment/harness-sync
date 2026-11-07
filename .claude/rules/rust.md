---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
  - "rust-toolchain.toml"
  - "clippy.toml"
---

<rust>

- Invoke the `rust:rust` skill before writing Rust. Invoke the `rust:rust-testing` skill before writing a test.
- Follow the skills for each question that this file does not answer. Where a skill and this file differ, follow this
  file.

<lints>

- `clippy.toml` sets strict limits on argument count, boolean parameters, function length, cognitive complexity, and
  nesting depth. Read it before you design a function signature.
- The `[workspace.lints]` table in the root `Cargo.toml` warns on `print_stdout` and `print_stderr`. Write terminal
  output only in `crates/harness-sync/src/output.rs`.

</lints>

<testing>

- `std::assert_matches` is unstable through Rust 1.99, and its import does not resolve. Write
  `assert!(matches!(value, Pattern))`. Do not write a `match` that ends in `panic!`.
- End-to-end tests in `crates/harness-sync/tests/` run the `harness-sync` binary with a temporary home directory and a
  bare git repo on disk. Never let a test read or write the real `~/.claude`, `~/.claude.json`, `~/.pi`, or
  `~/.harness-sync`.

</testing>

<versions>

- `rust-toolchain.toml` pins the toolchain for local builds and for CI. Change it in a commit of its own, and never name
  a moving channel such as `stable` in place of a version.
- Write each dependency requirement as the full version that `Cargo.lock` resolves, such as `clap = "4.6.7"`. Never
  write an `=` requirement.

</versions>

<verification>

- Run these gates before you report a change as done:
  - `cargo fmt --all`
  - `cargo clippy --locked --all-targets --all-features -- -D warnings`
  - `cargo test --locked`
- Run tests with `cargo test`, not `cargo nextest run`.

</verification>

</rust>
