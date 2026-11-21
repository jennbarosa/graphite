# Graphite

- All front ends must be in sync. When a behavior, option, default, or annotation changes in one
  front end (or in the durable engine they share), make the same change in every other front end,
  and update the docs, CLI help, and CHANGELOG that describe it, in the same change.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **graphite** (4447 symbols, 10121 relationships, 268 execution flows).

> Index stale? Run `node .gitnexus/run.cjs analyze --index-only` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? Bootstrap with `npx`, `bunx`, or `pnpm dlx` — e.g. `bunx gitnexus@latest analyze` (npm 11 npx crash; #1939).

## Always Do

- **MUST run impact before editing.** Use `impact({target: "symbolName", direction: "upstream"})` or `node .gitnexus/run.cjs impact "symbolName" --direction upstream --repo .`; report callers, processes, and risk. Never substitute grep for graph analysis.
- **MUST analyze graph changes before committing.** Use `detect_changes({scope: "all"})` (MCP) or `node .gitnexus/run.cjs detect-changes --scope all --repo .` (CLI fallback). `partial: true` or `truncated: true` is not a clean check — a zero means unseen, not unaffected; re-run it. For regression review: `detect_changes({scope: "compare", base_ref: "main"})` or `node .gitnexus/run.cjs detect-changes --scope compare --base-ref "main" --repo .`.
- MUST warn on HIGH/CRITICAL `risk` pre-edit; never use `riskSharedAxes` to waive a HIGH/CRITICAL `risk` warning. Compare File/symbol: MCP File omits axes; Graph-RAG expands File.
- **MUST treat `risk: UNKNOWN` as unresolved, not as low.** An empty caller set is not evidence the symbol is unused — it can also mean the callers are not resolvable by the index (plain-object property access, dynamic dispatch, cross-language calls). `impact` pairs `UNKNOWN` with a `riskNote` saying so. Confirm with a text search before treating the symbol as safe to change or delete; do not proceed on the strength of a zero.
- **MUST use `query({search_query: "concept"})` for concepts/flows, `context({name: "symbolName"})` for a named symbol, or `impact` for blast radius, on read-only callers, dependencies, imports, or execution flow.** Graph first; text search only for empty/`UNKNOWN`/literals.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method before MCP/CLI impact analysis.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis, and never read `UNKNOWN` as an all-clear — it means the walk could not answer, which is the one verdict that requires confirming by other means.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit before MCP/CLI graph change analysis.

## Resources

| Resource | Use for |
| --- | --- |
| `gitnexus://repo/graphite/context` | Codebase overview, check index freshness |
| `gitnexus://repo/graphite/clusters` | All functional areas |
| `gitnexus://repo/graphite/processes` | All execution flows |
| `gitnexus://repo/graphite/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
| --- | --- |
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

## Verification

Run these steps in order before calling a change done or committing. Each step must pass before the next one starts. They mirror the CI gates in `.github/workflows/ci.yml`.

1. **Format and index.**
   a. `cargo fmt` formats every file. Do not stop at `cargo fmt --check`; let the formatter write its changes.
   b. Re-index the graph: `node .gitnexus/run.cjs analyze --index-only`.
2. **Lint and unit tests.**
   a. `cargo clippy --all-targets -- -D warnings`
   b. Generated rule data is current:
      ```sh
      for rule in lex grammar structure directive; do
        cargo run --quiet --example bootstrap_dump -- "src/graphs/$rule.graphite" "src/generated/$rule.rs"
      done
      git diff --exit-code -- src/generated
      ```
   c. All unit tests: `cargo test --lib --bins`
3. **Integration tests and `.graphite` fixtures.**
   a. The fixture-driven targets, named explicitly so they are never skipped:
      ```sh
      cargo test --test parser_fixtures --test examples --test real_workflows \
        --test front_end_equivalence --test chain_equivalence \
        --test pure_frontends --test pure_graphs --test cli_pure
      ```
      When you add a fixture-driven test target, add it to this list.
   b. All integration tests: `cargo test` (the full suite; it includes every target above and the CLI tests that also touch `.graphite` files).
4. **Graph check and commit**, only when steps 1 to 3 all pass.
   a. `node .gitnexus/run.cjs detect-changes --scope all --repo .`. Report the risk level. `partial: true` or `truncated: true` is not clean; re-run it.
   b. Commit the changed files with a message that describes the change. **Do not add a `Co-Authored-By` trailer or any other co-author line.**

A failing gate is a failing change. Report the output, not a summary of it.
