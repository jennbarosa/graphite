//! Every way a machine graph and a pure graph differ inside the compiler, in one place. The parser,
//! the structure checks, the pipeline and the CLI ask a `Kind`, rather than testing a mode.

/// A body line of a graph, as the grammar sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Default,
    Node,
    Edge,
    Choose,
    Enum,
    Input,
}

/// What kind of graph a file holds. The set is closed and compiler-internal, so this is an enum
/// with methods rather than a trait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A state machine: nodes, edges and choices, run by the runtime.
    #[default]
    Machine,
    /// A pure decision table: enums, inputs and choices, compiled to plain functions.
    Pure,
}

const NODE_IN_PURE: &str = "`node` is not allowed in a pure file";
const EDGE_IN_PURE: &str = "`edge` is not allowed in a pure file";

/// Whether `message` is the refusal of a machine line (`node` or `edge`) in a pure file. The
/// pipeline uses it to add the reason the file is pure.
pub fn is_machine_line_refusal(message: &str) -> bool {
    message == NODE_IN_PURE || message == EDGE_IN_PURE
}

impl Kind {
    /// Whether a body line may appear in a graph of this kind. The error names the fix.
    pub fn allows(self, line: Line) -> Result<(), &'static str> {
        match (self, line) {
            (Kind::Machine, Line::Enum) => {
                Err("`enum` needs a pure file (use `--pure`, `@!pure`, or a `.pure.graphite` name)")
            }
            (Kind::Machine, Line::Input) => Err(
                "`input` needs a pure file (use `--pure`, `@!pure`, or a `.pure.graphite` name)",
            ),
            (Kind::Pure, Line::Node) => Err(NODE_IN_PURE),
            (Kind::Pure, Line::Edge) => Err(EDGE_IN_PURE),
            _ => Ok(()),
        }
    }

    /// The conditions the structure graph's first task returns for this kind.
    pub fn structure_route(self) -> &'static [&'static str] {
        match self {
            Kind::Machine => &["machine"],
            Kind::Pure => &["pure"],
        }
    }

    /// Whether `graphite run` and `graphite debug` can take this kind of graph.
    pub fn runnable(self) -> bool {
        match self {
            Kind::Machine => true,
            Kind::Pure => false,
        }
    }

    /// The word for this kind in diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Machine => "machine",
            Kind::Pure => "pure",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Line, NODE_IN_PURE};

    const LINES: [Line; 6] = [
        Line::Default,
        Line::Node,
        Line::Edge,
        Line::Choose,
        Line::Enum,
        Line::Input,
    ];

    #[test]
    fn machine_files_refuse_only_the_pure_lines() {
        let refused: Vec<Line> = LINES
            .into_iter()
            .filter(|&line| Kind::Machine.allows(line).is_err())
            .collect();
        assert_eq!(refused, [Line::Enum, Line::Input]);
    }

    #[test]
    fn pure_files_refuse_only_nodes_and_edges() {
        let refused: Vec<Line> = LINES
            .into_iter()
            .filter(|&line| Kind::Pure.allows(line).is_err())
            .collect();
        assert_eq!(refused, [Line::Node, Line::Edge]);
    }

    #[test]
    fn refusals_name_their_fix() {
        assert_eq!(
            Kind::Machine.allows(Line::Input),
            Err("`input` needs a pure file (use `--pure`, `@!pure`, or a `.pure.graphite` name)")
        );
        assert_eq!(Kind::Pure.allows(Line::Node), Err(NODE_IN_PURE));
    }

    #[test]
    fn structure_route_runnable_and_label_for_each_kind() {
        assert_eq!(Kind::Machine.structure_route(), ["machine"]);
        assert_eq!(Kind::Pure.structure_route(), ["pure"]);
        assert!(Kind::Machine.runnable());
        assert!(!Kind::Pure.runnable());
        assert_eq!(Kind::Machine.label(), "machine");
        assert_eq!(Kind::Pure.label(), "pure");
    }

    #[test]
    fn the_default_kind_is_machine() {
        assert_eq!(Kind::default(), Kind::Machine);
    }
}
