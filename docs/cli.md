# CLI reference

`graphite help` lists the commands. `graphite help <command>` or `graphite <command> --help` shows
one command's options.

```text
graphite init <file.graphite>                               write a starter graph
graphite check <file.graphite> [--allow <list.toml>]        validate a graph
graphite compile <file.graphite> [--out <dir>]              generate modules with a front end
graphite run <file.graphite> [options]                      run the first graph as a durable workflow
graphite debug <file.graphite> [command...]                 step through a graph in the terminal
graphite debug <file.graphite> --simulate <task>=<outcomes> check a graph's routes, with no task run
graphite inspect <file.graphite> [--json]                   print the compiled graph
graphite doctor [--json]                               report this build, its front ends, and what works here
graphite help [command]                                show help for a command
graphite --version                                     print the version
```

## Commands

| Command | Does | Writes files? |
| --- | --- | --- |
| `init <file>` | Writes a starter graph. Refuses to overwrite. | Yes, one file |
| `check <file>` | Validates the graph and prints `ok: N graphs`. | No |
| `compile <file>` | Generates the front end's modules into `--out`, default `graphite-out/<stem>/`. | Yes |
| `run <file>` | Runs the first graph as a durable workflow. See [durable workflows](durable-workflows.md). | State file only |
| `debug <file>` | Runs the debugger with the given commands, or reads them from stdin. With `--simulate`, runs the durable engine with stubbed task outcomes instead. | No |
| `inspect <file>` | Prints the compiled nodes, edges, conditions, choices and annotations. | No |
| `doctor` | Reports the graphite version, each front end, and whether `debug` and `run` can work here. | No |

## Options

`graphite help <command>` lists every option of a command and what it does. The main ones:

| Option | Commands | Meaning |
| --- | --- | --- |
| `--frontend <key>` | `check`, `compile`, `debug` | The front end to use; see [choosing the front end](#choosing-the-front-end-and-mode). |
| `--out <dir>` | `compile`, `debug` | Output directory. `compile` defaults to `graphite-out/<stem>`. |
| `--allow <list.toml>` | `check`, `compile`, `run`, `debug` | Allowlist of flags and annotation values. See [allowlists](dsl.md#allowlists). |
| `--simulate <task>=<outcomes>` | `debug` | Checks a graph's routes with stubbed outcomes. See [checking routes](durable-workflows.md#checking-routes-without-running-tasks). |
| `--state`, `--runner`, `--tasks`, `--input`, `--each`, `--retry` | `run` | See [durable workflows](durable-workflows.md). |

## Choosing the front end and mode

A file's front end comes from the first of these that applies:

1. `--frontend <key>`.
2. An `@!frontend:<key>` directive at the top of the file. A directive loses to the flag, and is
   then ignored completely.
3. The file's infix. `door.ts.graphite` uses the target whose extension is `ts`, and
   `door.rs.graphite` uses `rust`. The last infix counts, so `combat.ts.purs.graphite` is
   PureScript. A name without `.graphite` has no infix. `compile` and `check` refuse an infix no
   front end writes; the other commands fall back to the default.
4. The default: `typescript`.

A file's mode is the kind of its front end. A pure front end, such as `purescript`, makes the file
a pure file. Nothing else changes the mode.

| File | Front end | Mode |
| --- | --- | --- |
| `door.graphite` | `typescript` | machine |
| `door.rs.graphite` | `rust` | machine |
| `combat.purs.graphite` | `purescript` | pure |
| `door.graphite` with `@!frontend:purescript` | `purescript` | pure |
| `combat.pure.graphite` | error: no front end for the infix `.pure` | |
| `my.walk.graphite` | error: no front end for the infix `.walk` (`compile` and `check` only) | |

A machine line in a pure file names the front end that made it pure, as in "`node` is not allowed
in a pure file (the `purescript` front end writes pure files)".

## Exit status

Scripts can tell a bad invocation from a failed command:

| Status | Meaning |
| --- | --- |
| `0` | The command succeeded. |
| `1` | The command failed: a graph does not check, a run stalls, or a runtime is missing. |
| `2` | A usage error: an unknown command or option, or a missing file or value. |

Errors go to stderr. Help and `--version` go to stdout.

## JSON output

`check`, `inspect` and `doctor` take `--json` and print one JSON object on stdout, for scripts
and editors. Every object starts with `"ok"`:

| Command | Success | Failure |
| --- | --- | --- |
| `check --json` | `{"ok":true,"graphs":1}` | `{"ok":false,"error":"..."}`, exit 1 |
| `inspect --json` | `{"ok":true,"graphs":[{"name":...,"nodes":[...],"conditions":[...],"choices":[...],"edges":[...]}]}` | `{"ok":false,"error":"..."}`, exit 1 |
| `doctor --json` | `{"ok":true,"graphite":{...},"core":{...},"front_ends":[...],"blocked":[]}` | `"ok":false` with the `blocked` list, exit 1 |

On failure, `error` holds the same text the command prints to stderr without `--json`. Usage errors
(exit 2) still go to stderr, because the arguments were not understood and no JSON is produced.

In `inspect`, a choice arm's `condition` is `null` for `else`, and an annotation's `value` is `null`
for a bare annotation such as `@flip`. Lists keep the order the text form prints.

## Environment variables

| Variable | Effect |
| --- | --- |
| `GRAPHITE_JS_RUNNER` | The command used to run TypeScript, such as `bun` or `npx tsx`. Takes precedence over the default search. |

## Where the TypeScript comes from

The durable engine and the debugger are compiled into the `graphite` binary. Each run writes them
to a private temporary directory and removes it afterwards. An installed binary therefore works
anywhere; it does not need a checkout.
