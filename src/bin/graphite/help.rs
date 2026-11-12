//! Help text. `graphite help` and `graphite <command> --help` print these; errors print the overview.

pub const OVERVIEW: &str = "usage:
  graphite --version                                     print the version
  graphite init <file.dsl>                               write a starter graph
  graphite check <file.dsl> [--allow <list.toml>] [--json]  validate a graph
  graphite compile <file.dsl> [--out <dir>]              generate TypeScript modules
  graphite run <file.dsl> [--state <file.json>] [--runner <tasks.ts>] [--allow <list.toml>]
  graphite debug <file.dsl> [command...]                 step through a graph in the terminal
  graphite inspect <file.dsl> [--json]                   print the compiled graph
  graphite doctor [--json]                               report this build, its front ends, and what works here

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
            "usage: graphite check <file.dsl> [--frontend typescript] [--allow <list.toml>] [--json]

Parses and validates the graph, and checks that the generated file names do not collide.
Prints `ok: N graphs` when the file is valid. With --json, prints {\"ok\":true,\"graphs\":N}, or
{\"ok\":false,\"error\":...} on failure. Writes nothing."
        }
        "compile" => {
            "usage: graphite compile <file.dsl> [--out <dir>] [--frontend typescript] [--allow <list.toml>]

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
            "usage: graphite inspect <file.dsl> [--json]

Prints the compiled nodes, edges, conditions, choices and annotations, for checking what the
parser understood. --json prints the same facts as one JSON object."
        }
        "doctor" => {
            "usage: graphite doctor [--json]

Reports the graphite version and platform, the commands that need no outside tools, and each
registered front end: whether it compiles, and whether its debug and run have the runtime they
need. Each blocked command says how to fix it. Exits with an error when any command is blocked. --json prints the same report as one JSON object."
        }
        _ => return None,
    })
}
