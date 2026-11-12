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
graphite completions <bash|zsh|fish>                   print a shell completion script
graphite man                                           print the manual page
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
| `completions <shell>` | Prints a completion script for `bash`, `zsh` or `fish`. | No |
| `man` | Prints the manual page in roff. | No |

## Options

| Option | Commands | Meaning |
| --- | --- | --- |
| `--out <dir>` | `compile`, `debug` | Output directory. `compile` defaults to `graphite-out/<stem>`. `debug` uses a temporary directory. |
| `--allow <list.toml>` | `check`, `compile`, `run`, `debug` | Allowlist of flags and annotation values, as names only. It holds no commands. See [allowlists](dsl.md#allowlists). |
| `--graph <name>` | `debug` | Debug this graph instead of the file's first one. |
| `--simulate <task>=<outcomes>` | `debug` | Runs the graph under the durable engine with each task's outcome stubbed, and runs no task code. Repeatable. Takes no commands. Outcomes are `pass`, `fail`, `none` or `throw`. See [checking routes](durable-workflows.md#checking-routes-without-running-tasks). |
| `--state <file.json>` | `run` | Checkpoint file. Defaults to `<stem>.state.json` beside the file. |
| `--runner <tasks.ts>` | `run` | Module whose default export runs tasks. Runs as code with your permissions. |
| `--tasks <tasks.toml>` | `run` | Runs each task as a command, from a manifest whose `[tasks]` table holds the commands. Cannot be combined with `--runner`. See [command tasks](durable-workflows.md#command-tasks). |
| `--input <input.json>` | `run` | A JSON value every task receives as `context.input`. See [input](durable-workflows.md#input). |
| `--each <item.graphite>` | `run` | Runs the item graph once per item the graph produces. Needs `--key`. See [fan-out](durable-workflows.md#fan-out). |
| `--key <field>` | `run` | The item field that identifies each item. |
| `--seen <file.json>` | `run` | Item keys already started, so a run skips them. Defaults beside the state file. |
| `--until-passed <N>` | `run` | Stops starting items once N have passed. |
| `--durable` | `run` | Accepted and ignored: `run` is always durable. |
| `--frontend <key>` | `check`, `compile`, `debug` | The front end to use. `typescript` is the only one shipped, and the default. See [front ends](front-ends.md). |

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
| `doctor --json` | `{"ok":true,"graphite":{...},"core":{...},"front_ends":[...],"test_tools":[...],"blocked":[]}` | `"ok":false` with the `blocked` list, exit 1 |

On failure, `error` holds the same text the command prints to stderr without `--json`. Usage errors
(exit 2) still go to stderr, because the arguments were not understood and no JSON is produced.

In `inspect`, a choice arm's `condition` is `null` for `else`, and an annotation's `value` is `null`
for a bare annotation such as `@flip`. Lists keep the order the text form prints.

## Shell completions and the manual page

`graphite completions <bash|zsh|fish>` prints a completion script for the shell. The script is built
from the same command table as the parser, so it lists the commands and options the binary accepts.

```sh
graphite completions bash > ~/.local/share/bash-completion/completions/graphite
graphite completions zsh > "${fpath[1]}/_graphite"
graphite completions fish > ~/.config/fish/completions/graphite.fish
```

`graphite man` prints the manual page in roff. The command sections come from the same help text as
`graphite help`:

```sh
graphite man > ~/.local/share/man/man1/graphite.1
man graphite
```

## Environment variables

| Variable | Effect |
| --- | --- |
| `GRAPHITE_JS_RUNNER` | The command used to run TypeScript, such as `bun` or `npx tsx`. Takes precedence over the default search. |
| `GRAPHITE_TSC` | The path to `tsc`, used by the TypeScript type-check test. `doctor` reports it. |

## Where the TypeScript comes from

The durable engine and the debugger are compiled into the `graphite` binary. Each run writes them
to a private temporary directory and removes it afterwards. An installed binary therefore works
anywhere; it does not need a checkout.
