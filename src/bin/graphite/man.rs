//! `graphite man`: the manual page in roff. The command sections come from the same help text
//! that `graphite help` prints, so the page cannot disagree with the binary.

use crate::help::{self, COMMANDS};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn page() -> String {
    let mut out = format!(".TH GRAPHITE 1 \"\" \"graphite {VERSION}\" \"User Commands\"\n");
    section(&mut out, "NAME");
    out += "graphite \\- compile, check and run state graphs\n";

    section(&mut out, "SYNOPSIS");
    out += ".nf\n";
    for line in synopsis() {
        out += &escape(line);
        out += "\n";
    }
    out += ".fi\n";

    section(&mut out, "DESCRIPTION");
    out += &paragraph(
        "Graphite reads a state graph written in a small DSL and checks it when you build it. \
         Front ends generate code for host languages, and the durable host and debugger step the \
         graph one step at a time. The documentation is in the docs directory of the source \
         repository.",
    );

    section(&mut out, "COMMANDS");
    for (name, _) in COMMANDS {
        out += &format!(".SS {}\n", escape(&format!("graphite {name}")));
        let text = help::for_command(name).unwrap_or_default();
        for block in text.split("\n\n") {
            out += &paragraph(block);
        }
    }

    section(&mut out, "OPTIONS");
    out += ".TP\n\\fB\\-\\-version\\fR, \\fB\\-V\\fR\nPrint the version and exit.\n";

    section(&mut out, "EXIT STATUS");
    out += ".TP\n\\fB0\\fR\nThe command succeeded.\n";
    out += ".TP\n\\fB1\\fR\nThe command failed: a graph does not check, a run stalls, or a runtime is missing.\n";
    out += ".TP\n\\fB2\\fR\nA usage error: an unknown command or option, or a missing file or value.\n";

    section(&mut out, "ENVIRONMENT");
    out += ".TP\n\\fBGRAPHITE_JS_RUNNER\\fR\nThe command used to run TypeScript, such as bun or \"npx tsx\".\n";
    out += ".TP\n\\fBGRAPHITE_TSC\\fR\nThe path to tsc, used by the TypeScript type-check test.\n";
    out
}

/// The usage lines from the overview: everything after `usage:` and before the closing hint.
fn synopsis() -> Vec<&'static str> {
    let body = help::OVERVIEW
        .split_once("usage:\n")
        .map_or("", |(_, rest)| rest);
    body.split("\n\n").next().unwrap_or("").lines().collect()
}

fn section(out: &mut String, title: &str) {
    *out += &format!(".SH {title}\n");
}

/// A roff paragraph: `.PP`, then each line with its roff characters escaped.
fn paragraph(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(escape).collect();
    format!(".PP\n{}\n", lines.join("\n"))
}

/// Escapes what roff would read as markup: backslashes, hyphens (so they print as minus signs),
/// and a leading dot or quote that would start a request.
fn escape(line: &str) -> String {
    let mut out = String::new();
    if line.starts_with('.') || line.starts_with('\'') {
        out.push_str("\\&");
    }
    for ch in line.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '-' => out.push_str("\\-"),
            c => out.push(c),
        }
    }
    out
}
