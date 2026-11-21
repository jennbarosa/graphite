//! PureScript layout for a pure graph: one module with its types, enums, annotation values,
//! arm values and choice functions. Names and literals all come from `lang::PURESCRIPT`.
//!
//! Each choice's tag function is derived from one decision body: a dispatch `<choice>Arm` picks
//! an arm value, which carries its tag and its annotations, and the choice takes the tag from it.

use std::io::{self, Write};

use super::decision::{Case, Decisions, EnumDecl, Function, Notes, Test, Ty};
use super::{Generated, Pure, PureSyntax, Source};
use crate::frontend::lang::purescript::PURESCRIPT;
use crate::frontend::lang::{Language, Role};

/// The PureScript target's front end.
pub const TARGET: crate::frontend::registry::Target = crate::frontend::registry::Target {
    key: "purescript",
    frontend: &PURESCRIPT_FRONT_END,
    debugger: None,
    runtime: None,
};

static PURESCRIPT_FRONT_END: Pure<PureScriptSyntax> = Pure::new(PureScriptSyntax);

/// PureScript's layout of a module.
pub struct PureScriptSyntax;

/// Every name the layout emits for one module, spelled once. `generated` reports them to the
/// collision check and `module` writes them, so the two cannot disagree. An optional name is
/// `Some` exactly when `module` writes that value.
struct Layout<'d> {
    graph_annotations: bool,
    enums: Vec<EnumNames<'d>>,
    inputs: Vec<InputNames<'d>>,
    choices: Vec<ChoiceNames<'d>>,
}

struct EnumNames<'d> {
    source: Source<'d>,
    /// The type, such as `Dir`.
    ty: String,
    /// Each member's spelling, in source order.
    members: Vec<String>,
    enum_annotations: Option<String>,
    member_annotations: Option<String>,
}

struct InputNames<'d> {
    source: Source<'d>,
    /// The record field, such as `homeEast`.
    base: String,
    annotations: Option<String>,
}

struct ChoiceNames<'d> {
    source: Source<'d>,
    base: String,
    annotations: Option<String>,
    /// One value per arm, in source order.
    arms: Vec<String>,
    /// The function that picks an arm from the input, such as `tacticArm`.
    dispatch: String,
}

impl<'d> Layout<'d> {
    fn new(language: &dyn Language, decisions: &Decisions<'d>) -> Self {
        let enums = decisions
            .enums
            .iter()
            .map(|item| {
                let base = language.ident(Role::Constant, item.name);
                let any_member = item.members.iter().any(|member| !member.notes.is_empty());
                EnumNames {
                    source: Source {
                        name: item.name,
                        kind: "enum",
                        offset: item.offset,
                    },
                    ty: language.ident(Role::Type, item.name),
                    members: item
                        .members
                        .iter()
                        .map(|member| language.ident(Role::Member, member.name))
                        .collect(),
                    enum_annotations: (!item.notes.is_empty())
                        .then(|| format!("{base}EnumAnnotations")),
                    member_annotations: any_member.then(|| format!("{base}MemberAnnotations")),
                }
            })
            .collect();
        let inputs = decisions
            .inputs
            .iter()
            .map(|input| {
                let base = language.ident(Role::Field, input.name);
                InputNames {
                    source: Source {
                        name: input.name,
                        kind: "input",
                        offset: input.offset,
                    },
                    annotations: (!input.notes.is_empty()).then(|| format!("{base}Annotations")),
                    base,
                }
            })
            .collect();
        let choices = decisions
            .functions
            .iter()
            .map(|function| {
                let base = language.ident(Role::Function, function.name);
                ChoiceNames {
                    source: Source {
                        name: function.name,
                        kind: "choice",
                        offset: function.offset,
                    },
                    annotations: (!function.notes.is_empty()).then(|| format!("{base}Annotations")),
                    arms: (0..function.cases.len())
                        .map(|index| format!("{base}Arm{index}"))
                        .collect(),
                    dispatch: format!("{base}Arm"),
                    base,
                }
            })
            .collect();
        Self {
            graph_annotations: !decisions.notes.is_empty(),
            enums,
            inputs,
            choices,
        }
    }
}

/// A generated name that its declaration `source` produces, or that every module declares when
/// `source` is `None`.
fn generated_name<'d>(role: Role, name: &str, source: Option<Source<'d>>) -> Generated<'d> {
    Generated {
        role,
        name: name.to_owned(),
        source,
    }
}

/// Pushes the name when the layout writes it. An absent name is not generated.
fn push_optional<'d>(
    out: &mut Vec<Generated<'d>>,
    role: Role,
    name: Option<&str>,
    source: Option<Source<'d>>,
) {
    out.extend(name.map(|name| generated_name(role, name, source)));
}

impl PureSyntax for PureScriptSyntax {
    fn language(&self) -> &'static dyn Language {
        &PURESCRIPT
    }

    fn generated<'d>(&self, decisions: &Decisions<'d>) -> Vec<Generated<'d>> {
        let layout = Layout::new(self.language(), decisions);
        let mut out = vec![
            generated_name(Role::Type, "Annotation", None),
            generated_name(Role::Type, "Arm", None),
            generated_name(Role::Type, "Input", None),
        ];
        if layout.graph_annotations {
            out.push(generated_name(Role::Constant, "graphAnnotations", None));
        }
        for item in &layout.enums {
            let source = Some(item.source);
            push_optional(
                &mut out,
                Role::Constant,
                item.enum_annotations.as_deref(),
                source,
            );
            push_optional(
                &mut out,
                Role::Function,
                item.member_annotations.as_deref(),
                source,
            );
        }
        for input in &layout.inputs {
            let source = Some(input.source);
            push_optional(
                &mut out,
                Role::Constant,
                input.annotations.as_deref(),
                source,
            );
        }
        for choice in &layout.choices {
            let source = Some(choice.source);
            push_optional(
                &mut out,
                Role::Constant,
                choice.annotations.as_deref(),
                source,
            );
            out.extend(
                choice
                    .arms
                    .iter()
                    .map(|name| generated_name(Role::Constant, name, source)),
            );
            out.push(generated_name(Role::Function, &choice.dispatch, source));
        }
        out
    }

    /// Every name the module imports or uses from Prelude and `Data.Maybe`. Source names must not
    /// spell them, or the references in the module would be ambiguous.
    fn imported(&self) -> Vec<(Role, &'static str)> {
        // The module names: a graph named `prelude` would import itself. `Prim` is built in, and
        // its types are in scope in every module.
        let mut names = vec![(Role::Module, "Prelude"), (Role::Module, "Prim")];
        names.extend(
            [
                "Char",
                "Constraint",
                "Function",
                "Int",
                "Number",
                "Partial",
                "Record",
                "Symbol",
                "Type",
            ]
            .map(|name| (Role::Type, name)),
        );
        // The types and classes of Prelude, which `import Prelude` brings in: a local type with one
        // of these names shadows it, and `purs --strict` refuses the shadowing.
        names.extend(
            [
                "Applicative",
                "Apply",
                "Bind",
                "Boolean",
                "BooleanAlgebra",
                "Bounded",
                "Category",
                "CommutativeRing",
                "DivisionRing",
                "Discard",
                "Eq",
                "EuclideanRing",
                "Field",
                "Functor",
                "HeytingAlgebra",
                "Monad",
                "Monoid",
                "Ord",
                "Ordering",
                "Ring",
                "Semigroup",
                "Semigroupoid",
                "Semiring",
                "Show",
                "Unit",
                "Void",
                "Maybe",
                "String",
                "Array",
            ]
            .map(|name| (Role::Type, name)),
        );
        names.extend([
            (Role::Member, "Just"),
            (Role::Member, "Nothing"),
            (Role::Function, "otherwise"),
            // The parameter of every choice's dispatch function, which would shadow a choice named `i`.
            (Role::Function, "i"),
        ]);
        names
    }

    fn module(&self, decisions: &Decisions, output: &mut dyn Write) -> io::Result<()> {
        let language = self.language();
        let layout = Layout::new(language, decisions);
        let mut groups = Groups {
            output,
            first: true,
        };

        groups.push(&format!(
            "module {} where\n\nimport Prelude\n{}",
            language.ident(Role::Module, decisions.name),
            maybe_import(decisions)
        ))?;
        groups.push(&types(decisions, &layout))?;
        if layout.graph_annotations {
            groups.push(&value(language, "graphAnnotations", &decisions.notes)?)?;
        }
        for (item, names) in decisions.enums.iter().zip(&layout.enums) {
            groups.push(&enum_declaration(names))?;
            if let Some(name) = &names.enum_annotations {
                groups.push(&value(language, name, &item.notes)?)?;
            }
            if let Some(name) = &names.member_annotations {
                groups.push(&member_annotations(language, name, item, names)?)?;
            }
        }
        for (input, names) in decisions.inputs.iter().zip(&layout.inputs) {
            if let Some(name) = &names.annotations {
                groups.push(&value(language, name, &input.notes)?)?;
            }
        }
        for (function, names) in decisions.functions.iter().zip(&layout.choices) {
            choice(language, decisions, &layout, function, names, &mut groups)?;
        }
        groups.output.write_all(b"\n")
    }
}

/// Writes the module's groups one after another, each separated by a blank line, so each group
/// is written as soon as it is built and the module is never held in memory as a whole.
struct Groups<'o> {
    output: &'o mut dyn Write,
    first: bool,
}

impl Groups<'_> {
    fn push(&mut self, text: &str) -> io::Result<()> {
        if !self.first {
            self.output.write_all(b"\n\n")?;
        }
        self.first = false;
        self.output.write_all(text.as_bytes())
    }
}

/// The `Data.Maybe` import. The constructors are imported only when some annotation exists, since
/// `purs` warns about an imported constructor that nothing uses.
fn maybe_import(decisions: &Decisions) -> &'static str {
    let mut any = !decisions.notes.is_empty();
    any |= decisions.enums.iter().any(|item| {
        !item.notes.is_empty() || item.members.iter().any(|member| !member.notes.is_empty())
    });
    any |= decisions.inputs.iter().any(|input| !input.notes.is_empty());
    any |= decisions.functions.iter().any(|choice| {
        !choice.notes.is_empty() || choice.cases.iter().any(|case| !case.notes.is_empty())
    });
    if any {
        "import Data.Maybe (Maybe(..))"
    } else {
        "import Data.Maybe (Maybe)"
    }
}

/// The three types every module declares, and the input record.
fn types(decisions: &Decisions, layout: &Layout) -> String {
    let mut out = String::from(
        "type Annotation = { key :: String, value :: Maybe String }\n\
         type Arm t = { tag :: t, annotations :: Array Annotation }\n",
    );
    if decisions.inputs.is_empty() {
        out.push_str("type Input = {}");
        return out;
    }
    let fields: Vec<String> = decisions
        .inputs
        .iter()
        .zip(&layout.inputs)
        .map(|(input, names)| {
            let ty = match input.ty {
                Ty::Bool => "Boolean".to_owned(),
                Ty::Enum(index) => layout.enums[index].ty.clone(),
            };
            format!("{} :: {ty}", names.base)
        })
        .collect();
    out.push_str(&format!("type Input = {{ {} }}", fields.join(", ")));
    out
}

/// A value of type `Array Annotation`, named `name`, with the annotations of `notes`.
fn value(language: &dyn Language, name: &str, notes: &Notes) -> io::Result<String> {
    Ok(format!(
        "{name} :: Array Annotation\n{name} = {}",
        annotations(language, notes)?
    ))
}

/// The `data` line and its `Eq` instance. Its enum annotations are a separate group.
fn enum_declaration(names: &EnumNames) -> String {
    format!(
        "data {} = {}\nderive instance Eq {}",
        names.ty,
        names.members.join(" | "),
        names.ty
    )
}

/// `<name> :: T -> Array Annotation`, with one equation per member.
fn member_annotations(
    language: &dyn Language,
    name: &str,
    item: &EnumDecl,
    names: &EnumNames,
) -> io::Result<String> {
    let mut out = format!("{name} :: {} -> Array Annotation", names.ty);
    for (spelled, member) in names.members.iter().zip(&item.members) {
        out.push_str(&format!(
            "\n{name} {spelled} = {}",
            annotations(language, &member.notes)?
        ));
    }
    Ok(out)
}

/// The arm values, the dispatch that picks one, and the tag function of one choice.
fn choice(
    language: &dyn Language,
    decisions: &Decisions,
    layout: &Layout,
    function: &Function,
    names: &ChoiceNames,
    groups: &mut Groups,
) -> io::Result<()> {
    let enum_names = &layout.enums[function.returns];
    let ty = &enum_names.ty;
    if let Some(name) = &names.annotations {
        groups.push(&value(language, name, &function.notes)?)?;
    }
    let mut lines: Vec<String> = Vec::new();
    for (arm, case) in names.arms.iter().zip(&function.cases) {
        lines.push(format!("{arm} :: Arm {ty}"));
        let tag = &enum_names.members[case.member];
        lines.push(format!("{arm} = {}", arm_value(language, tag, case)?));
    }
    let dispatch = &names.dispatch;
    lines.push(format!("{dispatch} :: Input -> Arm {ty}"));
    lines.push(format!("{dispatch} i"));
    for (arm, case) in names.arms.iter().zip(&function.cases) {
        let guard = match case.test {
            None => "otherwise".to_owned(),
            Some(test) => guard(decisions, layout, test),
        };
        lines.push(format!("  | {guard} = {arm}"));
    }
    lines.push(format!("{} :: Input -> {ty}", names.base));
    lines.push(format!("{} = _.tag <<< {dispatch}", names.base));
    groups.push(&lines.join("\n"))
}

/// The record of an arm: its tag and its annotations.
fn arm_value(language: &dyn Language, tag: &str, case: &Case) -> io::Result<String> {
    Ok(format!(
        "{{ tag: {tag}, annotations: {} }}",
        annotations(language, &case.notes)?
    ))
}

/// The guard that tests an input: a bool field, or an equality with an enum member.
fn guard(decisions: &Decisions, layout: &Layout, test: Test) -> String {
    match test {
        Test::Flag(input) => format!("i.{}", layout.inputs[input].base),
        Test::Is(input, member) => {
            let Ty::Enum(index) = decisions.inputs[input].ty else {
                unreachable!("an `is` test names an enum input")
            };
            format!(
                "i.{} == {}",
                layout.inputs[input].base, layout.enums[index].members[member]
            )
        }
    }
}

/// A list literal of annotations, `[]` when there are none. One buffer holds the whole literal.
fn annotations(language: &dyn Language, notes: &Notes) -> io::Result<String> {
    if notes.is_empty() {
        return Ok("[]".to_owned());
    }
    let mut out: Vec<u8> = b"[ ".to_vec();
    for (index, (key, value)) in notes.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(b", ");
        }
        language.annotation(key, *value, &mut out)?;
    }
    out.extend_from_slice(b" ]");
    String::from_utf8(out).map_err(io::Error::other)
}
