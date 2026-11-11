//! Help text. `graphite help` and `graphite <command> --help` print these; errors print the overview.

pub const OVERVIEW: &str = "usage:
  graphite init <file.dsl>                               write a starter graph
  graphite check <file.dsl> [--allow <list.toml>]        validate a graph
  graphite compile <file.dsl> [--out <dir>]              generate TypeScript modules
  graphite run <file.dsl> [--state <file.json>] [--runner <tasks.ts>] [--allow <list.toml>]
  graphite debug <file.dsl> [command...]                 step through a graph in the terminal
  graphite inspect <file.dsl>                            print the compiled graph
  graphite doctor                                        check the JavaScript runtime and tsc

Run `graphite help <command>` for a command's options.";

/// The help for one command, or `None` for an unknown name.
pub fn for_command(name: &str) -> Option<&'static str> {
    Some(match name {
        "init" => {
            "usage: graphite init <file.dsl>

Writes a starter graph to <file.dsl>. It refuses to overwrite an existing file.
Next: `graphite check <file.dsl>`, then `graphite compile <file.dsl>`."
        }
        "check" => {
            "usage: graphite check <file.dsl> [--target typescript] [--allow <list.toml>]

Parses and validates the graph, and checks that the generated file names do not collide.
Prints `ok: N graphs` when the file is valid. Writes nothing."
        }
        "compile" => {
            "usage: graphite compile <file.dsl> [--out <dir>] [--target typescript] [--allow <list.toml>]

Writes one index module per graph, its chunk modules, and the runtime (graphite.ts).
--out defaults to graphite-out/<file name without extension>.
It never overwrites: remove the directory, or pass another --out, to compile again."
        }
        "run" => {
            "usage: graphite run <file.dsl> [--state <file.json>] [--runner <tasks.ts>] [--allow <list.toml>]

Runs the file's first graph as a durable workflow. Each node with a @task:NAME annotation runs
that task through --runner, and state is checkpointed after every result, so running the same
command again resumes where it stopped.
--state defaults to <file stem>.state.json beside the file.
--durable is accepted for compatibility and does nothing: `run` is always durable.
The runner is code that runs with your permissions. Use only runners you wrote or reviewed."
        }
        "debug" => {
            "usage: graphite debug <file.dsl> [--graph <name>] [--out <dir>] [--allow <list.toml>] [command...]

Opens a debugger on the first graph, or on --graph. Each argument after the file is one
command. Without commands, the debugger reads them from standard input.
Commands: show, set <cond> on|off, finish, step, choose <choice>, history, reset."
        }
        "inspect" => {
            "usage: graphite inspect <file.dsl>

Prints the compiled nodes, edges, conditions, choices and annotations, for checking what the
parser understood."
        }
        "doctor" => {
            "usage: graphite doctor

Reports which JavaScript runtime and TypeScript compiler graphite will use, and why a command
might fail. Exits with an error when no JavaScript runtime is found."
        }
        _ => return None,
    })
}
