//! Lowers a pure graph to a neutral form that every pure syntax reads. Syntaxes never touch
//! `CompiledGraph`, so the arrays and their spans stay inside this module and the parser.

use crate::graph::{CompiledGraph, Span, Test as GraphTest, Ty as GraphTy, index};
use crate::interner::StringId;

/// Annotations with their values resolved to text. A `None` value is an annotation with no value.
pub type Notes<'g> = Vec<(&'g str, Option<&'g str>)>;

/// One pure graph: its name, its declarations, and its choices as functions. `offset` is the
/// graph's, and every declaration carries its own, so a name clash can point at its line.
pub struct Decisions<'g> {
    pub name: &'g str,
    pub offset: usize,
    pub notes: Notes<'g>,
    pub enums: Vec<EnumDecl<'g>>,
    pub inputs: Vec<InputDecl<'g>>,
    pub functions: Vec<Function<'g>>,
}

pub struct EnumDecl<'g> {
    pub name: &'g str,
    pub offset: usize,
    pub notes: Notes<'g>,
    pub members: Vec<MemberDecl<'g>>,
}

pub struct MemberDecl<'g> {
    pub name: &'g str,
    pub offset: usize,
    pub notes: Notes<'g>,
}

pub struct InputDecl<'g> {
    pub name: &'g str,
    pub offset: usize,
    pub ty: Ty,
    pub notes: Notes<'g>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    /// An index into `Decisions::enums`.
    Enum(usize),
}

/// A test on an input. `Is` names the member by its position in the enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    /// A bool input is true.
    Flag(usize),
    /// `(input, member)`: an enum input equals one of its members.
    Is(usize, usize),
}

/// One arm of a choice: its test, the tag it returns, and its annotations.
pub struct Case<'g> {
    /// `None` for the final `else` arm.
    pub test: Option<Test>,
    pub tag: &'g str,
    /// The position of `tag` among its enum's members.
    pub member: usize,
    pub notes: Notes<'g>,
}

/// One choice: the enum it returns, its annotations, and its arms in source order. The last arm
/// is the `else`.
pub struct Function<'g> {
    pub name: &'g str,
    pub offset: usize,
    /// The index in `Decisions::enums` of the enum the choice returns.
    pub returns: usize,
    pub notes: Notes<'g>,
    pub cases: Vec<Case<'g>>,
}

/// Lowers a pure graph. Every declaration keeps its notes, so no annotation is lost on the way.
pub fn lower(graph: &CompiledGraph) -> Decisions<'_> {
    Decisions {
        name: graph.text(graph.name),
        offset: graph.offset,
        notes: notes(graph, &graph.graph_annotations),
        enums: graph
            .enums
            .iter()
            .map(|item| EnumDecl {
                name: graph.text(item.name),
                offset: item.offset,
                notes: notes(graph, &item.annotations),
                members: graph.members[index(&item.members)]
                    .iter()
                    .map(|member| MemberDecl {
                        name: graph.text(member.name),
                        offset: member.offset,
                        notes: notes(graph, &member.annotations),
                    })
                    .collect(),
            })
            .collect(),
        inputs: graph
            .inputs
            .iter()
            .map(|input| InputDecl {
                name: graph.text(input.name),
                offset: input.offset,
                ty: match input.ty {
                    GraphTy::Bool => Ty::Bool,
                    GraphTy::Enum(index) => Ty::Enum(index as usize),
                },
                notes: notes(graph, &input.annotations),
            })
            .collect(),
        functions: graph
            .choices
            .iter()
            .map(|choice| Function {
                name: graph.text(choice.name),
                offset: choice.offset,
                returns: choice.ty.expect("a pure choice names its enum") as usize,
                notes: notes(graph, &choice.annotations),
                cases: graph.arms[index(&choice.arms)]
                    .iter()
                    .map(|arm| Case {
                        test: arm
                            .condition
                            .map(|condition| test(graph.tests[condition as usize])),
                        tag: graph.text(arm.tag),
                        member: member_position(graph, choice.ty, arm.tag),
                        notes: notes(graph, &arm.annotations),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The position of `tag` among the members of the enum `ty`. The parser refuses a tag that is not a
/// member, so a pure choice always finds it.
fn member_position(graph: &CompiledGraph, ty: Option<u32>, tag: StringId) -> usize {
    let ty = ty.expect("a pure choice names its enum");
    graph.members[index(&graph.enums[ty as usize].members)]
        .iter()
        .position(|member| member.name == tag)
        .expect("a pure arm's tag is a member of its choice's enum")
}

fn test(test: GraphTest) -> Test {
    match test {
        GraphTest::Flag(input) => Test::Flag(input as usize),
        GraphTest::Is(input, member) => Test::Is(input as usize, member as usize),
    }
}

fn notes<'g>(graph: &'g CompiledGraph, span: &Span) -> Notes<'g> {
    graph.annotations[index(span)]
        .iter()
        .map(|entry| {
            (
                graph.text(entry.key),
                entry.value.map(|value| graph.text(value)),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Test, Ty, lower};
    use crate::parser::Schema;
    use crate::pipeline::compile;

    fn lowered(source: &str) -> Vec<(String, usize)> {
        let graphs = compile(
            source,
            &Schema {
                pure: true,
                ..Schema::default()
            },
        )
        .unwrap();
        let decisions = lower(&graphs[0]);
        decisions
            .functions
            .iter()
            .map(|function| (function.name.to_owned(), function.cases.len()))
            .collect()
    }

    #[test]
    fn each_choice_becomes_a_function_with_its_arms_in_order() {
        let source = "graph g\n  enum E a b\n  input x : bool\n  input e : E\n  choose c : E\n    when x -> a\n    when e:b -> b @k:v\n    else -> a\n";
        assert_eq!(lowered(source), [("c".to_owned(), 3)]);
    }

    #[test]
    fn tests_and_types_are_positional() {
        let source = "graph g\n  enum E a b\n  input x : bool\n  input e : E\n  choose c : E\n    when e:b -> b\n    when x -> a\n    else -> a\n";
        let graphs = compile(
            source,
            &Schema {
                pure: true,
                ..Schema::default()
            },
        )
        .unwrap();
        let decisions = lower(&graphs[0]);
        assert_eq!(decisions.inputs[0].ty, Ty::Bool);
        assert_eq!(decisions.inputs[1].ty, Ty::Enum(0));
        let cases = &decisions.functions[0].cases;
        assert_eq!(cases[0].test, Some(Test::Is(1, 1)));
        assert_eq!(cases[1].test, Some(Test::Flag(0)));
        assert_eq!(cases[2].test, None);
        assert_eq!(decisions.functions[0].returns, 0);
    }

    #[test]
    fn notes_keep_their_values_and_their_absence() {
        let source = "graph g @owner:ai\n  enum E a\n  input x : bool @source\n  choose c : E\n    when x -> a @anim:run\n    else -> a\n";
        let graphs = compile(
            source,
            &Schema {
                pure: true,
                ..Schema::default()
            },
        )
        .unwrap();
        let decisions = lower(&graphs[0]);
        assert_eq!(decisions.notes, [("owner", Some("ai"))]);
        assert_eq!(decisions.inputs[0].notes, [("source", None)]);
        assert_eq!(
            decisions.functions[0].cases[0].notes,
            [("anim", Some("run"))]
        );
    }
}
