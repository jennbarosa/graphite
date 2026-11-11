# CLI reference

`graphite help` lists the commands. `graphite help <command>` or `graphite <command> --help` shows
one command's options.

```text
graphite init <file.dsl>                               write a starter graph
graphite check <file.dsl> [--allow <list.toml>]        validate a graph
graphite compile <file.dsl> [--out <dir>]              generate modules with a front end
graphite run <file.dsl> [--state <file.json>] [--runner <tasks.ts>] [--allow <list.toml>]
graphite debug <file.dsl> [command...]                 step through a graph in the terminal
graphite inspect <file.dsl>                            print the compiled graph
graphite doctor                                        check the JavaScript runtime and tsc
```

## Commands

| Command | Does | Writes files? |
| --- | --- | --- |
| `init <file>` | Writes a starter graph. Refuses to overwrite. | Yes, one file |
| `check <file>` | Validates the graph and prints `ok: N graphs`. | No |
| `compile <file>` | Generates the front end's modules into `--out`, default `graphite-out/<stem>/`. | Yes |
| `run <file>` | Runs the first graph as a durable workflow. See [durable workflows](durable-workflows.md). | State file only |
| `debug <file>` | Runs the debugger with the given commands, or reads them from stdin. | No |
| `inspect <file>` | Prints the compiled nodes, edges, conditions, choices and annotations. | No |
| `doctor` | Reports the JavaScript runtime and TypeScript compiler, and whether they work. | No |

## Options

| Option | Commands | Meaning |
| --- | --- | --- |
| `--out <dir>` | `compile`, `debug` | Output directory. `compile` defaults to `graphite-out/<stem>`. `debug` uses a temporary directory. |
| `--allow <list.toml>` | `check`, `compile`, `run`, `debug` | Allowlist of flags and annotation values. See [allowlists](dsl.md#allowlists). |
| `--graph <name>` | `debug` | Debug this graph instead of the file's first one. |
| `--state <file.json>` | `run` | Checkpoint file. Defaults to `<stem>.state.json` beside the file. |
| `--runner <tasks.ts>` | `run` | Module whose default export runs tasks. Runs as code with your permissions. |
| `--durable` | `run` | Accepted and ignored: `run` is always durable. |
| `--target <key>` | `check`, `compile`, `debug` | The front end to use. `typescript` is the only one shipped, and the default. See [front ends](front-ends.md). |

## Exit status

`0` when the command succeeds. `1` on any error, including a graph that does not check, a run that
stalls, or a missing runtime. Errors go to stderr. Help goes to stdout.

## Environment variables

| Variable | Effect |
| --- | --- |
| `GRAPHITE_JS_RUNNER` | The command used to run TypeScript, such as `bun` or `npx tsx`. Takes precedence over the default search. |
| `GRAPHITE_TSC` | The path to `tsc`, used by the test suite and `doctor`. |

## Where the TypeScript comes from

The durable host and the debugger are compiled into the `graphite` binary. Each run writes them
to a private temporary directory and removes it afterwards. An installed binary therefore works
anywhere; it does not need a checkout.
