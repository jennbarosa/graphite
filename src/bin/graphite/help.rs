//! Help text. `graphite help` and `graphite <command> --help` print these; errors print the overview.

pub const OVERVIEW: &str = "usage:
  graphite --version                                     print the version
  graphite init <file.dsl>                               write a starter graph
  graphite check <file.dsl> [--allow <list.toml>] [--json]  validate a graph
  graphite compile <file.dsl> [--out <dir>]              generate TypeScript modules
  graphite run <file.dsl> [--state <file.json>] [--tasks <tasks.toml> | --runner <tasks.ts>] [--input <data.json>] [--allow <list.toml>]
  graphite debug <file.dsl> [command...]                 step through a graph in the terminal
  graphite inspect <file.dsl> [--json]                   print the compiled graph
  graphite doctor [--json]                               report this build, its front ends, and what works here
  graphite completions <bash|zsh|fish>                  print a shell completion script
  graphite man                                          print the manual page

Run `graphite help <command>` for a command's options.";

/// Every command, and the options it takes. Completions and the manual page are built from this
/// table; a unit test checks each option against the command's help text.
pub const COMMANDS: &[(&str, &[&str])] = &[
    ("init", &[]),
    ("check", &["--frontend", "--allow", "--json"]),
    ("compile", &["--out", "--frontend", "--allow"]),
    ("inspect", &["--json"]),
    ("debug", &["--frontend", "--graph", "--out", "--allow"]),
    (
        "run",
        &[
            "--state",
            "--runner",
            "--tasks",
            "--input",
            "--each",
            "--key",
            "--seen",
            "--until-passed",
            "--allow",
            "--durable",
        ],
    ),
    ("doctor", &["--json"]),
    ("help", &[]),
    ("completions", &[]),
    ("man", &[]),
];

/// Options that take a value on the next argument.
pub fn takes_value(option: &str) -> bool {
    matches!(
        option,
        "--out"
            | "--frontend"
            | "--graph"
            | "--allow"
            | "--state"
            | "--runner"
            | "--tasks"
            | "--input"
            | "--each"
            | "--key"
            | "--seen"
            | "--until-passed"
    )
}

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
            "usage: graphite run <file.dsl> [--state <file.json>] [--tasks <tasks.toml> | --runner <tasks.ts>] [--input <data.json>] [--allow <list.toml>]
                   [--each <item.dsl> --key <field> [--seen <file.json>] [--until-passed <N>]]

Runs the file's first graph as a durable workflow. Each node with a @task:NAME annotation runs
that task through --runner, or through the command --tasks names for it, and state is checkpointed
after every result, so running the same command again resumes where it stopped.
--input <data.json> is a JSON value every task receives as context.input. A resume must give the
same input.
With --tasks, the manifest's [tasks] keys are the allowed task names, so --allow needs only node_flags.
--each <item.dsl> runs that graph once per item the file's graph ends with, as an array. --key names
the item field that identifies it. --seen keeps the keys already started, so a second run skips them.
--until-passed <N> stops starting items once N have ended on a passing stage.
--state defaults to <file stem>.state.json beside the file.
--durable is accepted for compatibility and does nothing: `run` is always durable.
The runner is code that runs with your permissions. Use only runners you wrote or reviewed."
        }
        "debug" => {
            "usage: graphite debug <file.dsl> [--frontend typescript] [--graph <name>] [--out <dir>] [--allow <list.toml>] [command...]

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
        "help" => {
            "usage: graphite help [command]

Prints the overview, or the help for one command. The same as `graphite <command> --help`."
        }
        "completions" => {
            "usage: graphite completions <bash|zsh|fish>

Prints a completion script for the shell. Install it where your shell looks for completions, for
example `graphite completions bash > ~/.local/share/bash-completion/completions/graphite`."
        }
        "man" => {
            "usage: graphite man

Prints the manual page in roff. Install it on your MANPATH, for example
`graphite man > ~/.local/share/man/man1/graphite.1`, then run `man graphite`."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, for_command};

    /// Each command has help text, and the help text names every option the table lists for it.
    #[test]
    fn the_command_table_matches_the_help_text() {
        for (name, options) in COMMANDS {
            let text = for_command(name).unwrap_or_else(|| panic!("no help for {name}"));
            for option in *options {
                assert!(
                    text.contains(option),
                    "{name}'s help does not mention {option}"
                );
            }
        }
    }
}
