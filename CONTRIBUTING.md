# Contributing to Graphite

Thanks for helping. Graphite is small, and most useful changes are a focused fix, a
test, or a clearer error message.

## Before you start

For anything bigger than a bug fix or a small change, open an issue first to discuss
the approach. This keeps the core small and avoids work that won't be merged. The core
deliberately knows nothing about games, animation, or workflows. Domain concepts belong
in annotations, front ends, or hosts.

## Setup

You need:

- Rust 1.88 or newer (edition 2024)
- [Bun](https://bun.sh) 1.4.4, for the TypeScript runtime and its tests
- TypeScript (`tsc`) 5 or later, for the type-check test. CI uses 6.0.3. Set
  `GRAPHITE_TSC` to its path, or put it on your `PATH`.

Build and run the tests from the repository root:

```sh
cargo build
cargo test
cargo test --no-default-features   # without the SIMD lexer
```

Several tests run TypeScript with Bun and fail with a clear message when Bun or `tsc`
is missing.

## Checks before you open a pull request

CI runs these on every push and pull request. Run them locally first:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --no-default-features
```

Clippy warnings fail the build, so fix them rather than adding `#[allow]` unless there
is a clear reason, and say that reason in a comment.

## Guidelines

- **Keep changes small and complete.** Fix the root cause, and update the callers,
  tests, and docs that the change affects.
- **Add a test for new behavior.** Parser changes usually belong in a fixture under
  `tests/fixtures/parser/`. The `.expected` files are hand-written, so update them by
  reasoning about the expected output, not by copying what the parser prints.
- **Keep errors source-located.** Compile errors should point at the line and column
  that caused them.
- **Don't add dependencies for small helpers.** Check whether the standard library or
  existing code already covers the need.
- **Keep the hot path allocation-free.** `tests/allocations.rs` enforces this for the
  step path. If you change it, run that test.

## Pull requests

- Branch from `main`, and keep each pull request to one logical change.
- Describe what changed and why. Link the issue if there is one.
- Make sure the checks above pass before requesting review.
- Commit messages should be short and describe the change in the imperative, such as
  `Reject duplicate edge flags`.

## Licensing

Graphite is licensed under the GNU Affero General Public License v3.0 (see `LICENSE`).
By contributing, you agree that your contribution is licensed under the same terms.

## Reporting security problems

Don't open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).
