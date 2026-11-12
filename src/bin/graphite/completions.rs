//! `graphite completions <shell>`: a completion script built from the command table in `help`,
//! so it lists the commands and options the parser accepts.

use crate::help::{COMMANDS, takes_value};

/// The script for `shell`, or `None` for a shell this build does not know.
pub fn script(shell: &str) -> Option<String> {
    match shell {
        "bash" => Some(bash()),
        "zsh" => Some(zsh()),
        "fish" => Some(fish()),
        _ => None,
    }
}

/// Every command name, space-separated.
fn names() -> String {
    let names: Vec<&str> = COMMANDS.iter().map(|(name, _)| *name).collect();
    names.join(" ")
}

fn bash() -> String {
    let mut cases = String::new();
    for (name, options) in COMMANDS {
        cases += &format!("        {name}) opts=\"{}\" ;;\n", options.join(" "));
    }
    format!(
        r#"# bash completion for graphite. Install with:
#   graphite completions bash > ~/.local/share/bash-completion/completions/graphite
_graphite() {{
    local cur="${{COMP_WORDS[COMP_CWORD]}}"
    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=($(compgen -W "{names} --version --help" -- "$cur"))
        return
    fi
    local opts=""
    case "${{COMP_WORDS[1]}}" in
{cases}    esac
    COMPREPLY=($(compgen -W "$opts" -- "$cur"))
    if [ -z "${{COMPREPLY[*]}}" ] && [[ "$cur" != -* ]]; then
        COMPREPLY=($(compgen -f -- "$cur"))
    fi
}}
complete -F _graphite graphite
"#,
        names = names(),
    )
}

fn zsh() -> String {
    let mut cases = String::new();
    for (name, options) in COMMANDS {
        let body = match *name {
            "completions" => "compadd -- bash zsh fish".to_string(),
            "help" => format!("compadd -- {}", names()),
            "man" => ":".to_string(),
            _ => {
                let specs: Vec<String> = options.iter().map(|option| zsh_spec(option)).collect();
                format!("_arguments {} '*:file:_files'", specs.join(" "))
            }
        };
        cases += &format!("        {name}) {body} ;;\n");
    }
    format!(
        r#"#compdef graphite
# zsh completion for graphite. Install with:
#   graphite completions zsh > "${{fpath[1]}}/_graphite"
_graphite() {{
    if (( CURRENT == 2 )); then
        compadd -- {names} --version --help
        return
    fi
    case $words[2] in
{cases}    esac
}}
_graphite "$@"
"#,
        names = names(),
    )
}

/// One `_arguments` spec for an option: a value option takes the next word as a file path.
fn zsh_spec(option: &str) -> String {
    if takes_value(option) {
        format!("'{option}[value]:value:_files'")
    } else {
        format!("'{option}[{}]'", option.trim_start_matches("--"))
    }
}

fn fish() -> String {
    let mut out = String::from(
        "# fish completion for graphite. Install with:\n#   graphite completions fish > ~/.config/fish/completions/graphite.fish\ncomplete -c graphite -f\n",
    );
    out += &format!(
        "complete -c graphite -n __fish_use_subcommand -a \"{}\"\n",
        names()
    );
    for (name, options) in COMMANDS {
        for option in *options {
            let long = option.trim_start_matches("--");
            let value = if takes_value(option) { " -r -F" } else { "" };
            out += &format!(
                "complete -c graphite -n \"__fish_seen_subcommand_from {name}\" -l {long}{value}\n"
            );
        }
        if *name == "completions" {
            out += "complete -c graphite -n \"__fish_seen_subcommand_from completions\" -a \"bash zsh fish\"\n";
        }
    }
    out
}
