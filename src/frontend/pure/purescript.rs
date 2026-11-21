//! PureScript layout for a pure graph: one module with its types, enums, annotation values,
//! arm values and choice functions. Names and literals all come from `lang::PURESCRIPT`.
//!
//! Each choice's tag function is derived from one decision body: a dispatch `<choice>Arm` picks
//! an arm value, which carries its tag and its annotations, and the choice takes the tag from it.

use std::io::{self, Write};

use super::decision::{Case, Decisions, Notes, Test, Ty};
use super::{Pure, PureSyntax};
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

/// The names the layout emits for a choice, from one place so `generated` and `module` agree.
struct ChoiceNames {
    base: String,
}

impl ChoiceNames {
    fn new(language: &dyn Language, name: &str) -> Self {
        Self {
            base: language.ident(Role::Function, name),
        }
    }

    fn annotations(&self) -> String {
        format!("{}Annotations", self.base)
    }

    fn arm(&self, index: usize) -> String {
        format!("{}Arm{index}", self.base)
    }

    fn dispatch(&self) -> String {
        format!("{}Arm", self.base)
    }
}

/// The base of an enum's generated values, such as `dir` for `Dir`.
fn enum_base(language: &dyn Language, name: &str) -> String {
    language.ident(Role::Constant, name)
}

/// The base of an input's generated value, such as `homeEast`.
fn input_base(language: &dyn Language, name: &str) -> String {
    language.ident(Role::Field, name)
}

impl PureSyntax for PureScriptSyntax {
    fn language(&self) -> &'static dyn Language {
        &PURESCRIPT
    }

    fn generated(&self, decisions: &Decisions) -> Vec<(Role, String)> {
        let language = self.language();
        let mut names = vec![
            (Role::Type, "Annotation".to_owned()),
            (Role::Type, "Arm".to_owned()),
            (Role::Type, "Input".to_owned()),
        ];
        if !decisions.notes.is_empty() {
            names.push((Role::Constant, "graphAnnotations".to_owned()));
        }
        for item in &decisions.enums {
            let base = enum_base(language, item.name);
            if !item.notes.is_empty() {
                names.push((Role::Constant, format!("{base}EnumAnnotations")));
            }
            if item.members.iter().any(|(_, notes)| !notes.is_empty()) {
                names.push((Role::Function, format!("{base}MemberAnnotations")));
            }
        }
        for input in &decisions.inputs {
            if !input.notes.is_empty() {
                let base = input_base(language, input.name);
                names.push((Role::Constant, format!("{base}Annotations")));
            }
        }
        for function in &decisions.functions {
            let names_of = ChoiceNames::new(language, function.name);
            if !function.notes.is_empty() {
                names.push((Role::Constant, names_of.annotations()));
            }
            for index in 0..function.cases.len() {
                names.push((Role::Constant, names_of.arm(index)));
            }
            names.push((Role::Function, names_of.dispatch()));
        }
        names
    }

    fn module(&self, decisions: &Decisions, output: &mut dyn Write) -> io::Result<()> {
        let language = self.language();
        let mut groups: Vec<String> = Vec::new();

        groups.push(format!(
            "module {} where\n\nimport Prelude\nimport Data.Maybe (Maybe(..))",
            language.ident(Role::Module, decisions.name)
        ));
        groups.push(types(language, decisions)?);
        if !decisions.notes.is_empty() {
            groups.push(value(language, "graphAnnotations", &decisions.notes)?);
        }
        for item in &decisions.enums {
            groups.push(enum_declaration(language, item)?);
            let base = enum_base(language, item.name);
            if !item.notes.is_empty() {
                groups.push(value(
                    language,
                    &format!("{base}EnumAnnotations"),
                    &item.notes,
                )?);
            }
            if item.members.iter().any(|(_, notes)| !notes.is_empty()) {
                groups.push(member_annotations(language, &base, item)?);
            }
        }
        for input in &decisions.inputs {
            if !input.notes.is_empty() {
                let base = input_base(language, input.name);
                groups.push(value(
                    language,
                    &format!("{base}Annotations"),
                    &input.notes,
                )?);
            }
        }
        for function in &decisions.functions {
            groups.extend(choice(language, decisions, function)?);
        }

        let text = groups.join("\n\n");
        output.write_all(text.as_bytes())?;
        output.write_all(b"\n")
    }
}

/// The three types every module declares, and the input record.
fn types(language: &dyn Language, decisions: &Decisions) -> io::Result<String> {
    let mut out = String::from(
        "type Annotation = { key :: String, value :: Maybe String }\n\
         type Arm t = { tag :: t, annotations :: Array Annotation }\n",
    );
    if decisions.inputs.is_empty() {
        out.push_str("type Input = {}");
        return Ok(out);
    }
    let fields: Vec<String> = decisions
        .inputs
        .iter()
        .map(|input| {
            let ty = match input.ty {
                Ty::Bool => "Boolean".to_owned(),
                Ty::Enum(index) => language.ident(Role::Type, decisions.enums[index].name),
            };
            format!("{} :: {ty}", input_base(language, input.name))
        })
        .collect();
    out.push_str(&format!("type Input = {{ {} }}", fields.join(", ")));
    Ok(out)
}

/// A value of type `Array Annotation`, named `name`, with the annotations of `notes`.
fn value(language: &dyn Language, name: &str, notes: &Notes) -> io::Result<String> {
    Ok(format!(
        "{name} :: Array Annotation\n{name} = {}",
        annotations(language, notes)?
    ))
}

/// The `data` line and its `Eq` instance. Its enum annotations are a separate group.
fn enum_declaration(
    language: &dyn Language,
    item: &super::decision::EnumDecl,
) -> io::Result<String> {
    let name = language.ident(Role::Type, item.name);
    let members: Vec<String> = item
        .members
        .iter()
        .map(|(member, _)| language.ident(Role::Member, member))
        .collect();
    Ok(format!(
        "data {name} = {}\nderive instance Eq {name}",
        members.join(" | ")
    ))
}

/// `<base>MemberAnnotations :: T -> Array Annotation`, with one equation per member.
fn member_annotations(
    language: &dyn Language,
    base: &str,
    item: &super::decision::EnumDecl,
) -> io::Result<String> {
    let name = language.ident(Role::Type, item.name);
    let mut out = format!("{base}MemberAnnotations :: {name} -> Array Annotation");
    for (member, notes) in &item.members {
        out.push_str(&format!(
            "\n{base}MemberAnnotations {} = {}",
            language.ident(Role::Member, member),
            annotations(language, notes)?
        ));
    }
    Ok(out)
}

/// One choice, as groups: its annotations (when it has any), then its arm values, the dispatch on
/// the input, and the function.
fn choice(
    language: &dyn Language,
    decisions: &Decisions,
    function: &super::decision::Function,
) -> io::Result<Vec<String>> {
    let names = ChoiceNames::new(language, function.name);
    let ty = language.ident(
        Role::Type,
        decisions.enums[function.returns.expect("a pure choice names its enum")].name,
    );
    let mut groups = Vec::new();
    if !function.notes.is_empty() {
        groups.push(value(language, &names.annotations(), &function.notes)?);
    }
    let mut lines: Vec<String> = Vec::new();
    for (index, case) in function.cases.iter().enumerate() {
        lines.push(format!("{} :: Arm {ty}", names.arm(index)));
        lines.push(format!(
            "{} = {}",
            names.arm(index),
            arm_value(language, case)?
        ));
    }
    lines.push(format!("{} :: Input -> Arm {ty}", names.dispatch()));
    lines.push(format!("{} i", names.dispatch()));
    for (index, case) in function.cases.iter().enumerate() {
        let guard = match case.test {
            None => "otherwise".to_owned(),
            Some(test) => guard(language, decisions, test),
        };
        lines.push(format!("  | {guard} = {}", names.arm(index)));
    }
    lines.push(format!("{} :: Input -> {ty}", names.base));
    lines.push(format!("{} = _.tag <<< {}", names.base, names.dispatch()));
    groups.push(lines.join("\n"));
    Ok(groups)
}

/// The record of an arm: its tag and its annotations.
fn arm_value(language: &dyn Language, case: &Case) -> io::Result<String> {
    Ok(format!(
        "{{ tag: {}, annotations: {} }}",
        language.ident(Role::Member, case.tag),
        annotations(language, &case.notes)?
    ))
}

/// The guard that tests an input: a bool field, or an equality with an enum member.
fn guard(language: &dyn Language, decisions: &Decisions, test: Test) -> String {
    match test {
        Test::Flag(input) => format!("i.{}", input_base(language, decisions.inputs[input].name)),
        Test::Is(input, member) => {
            let Ty::Enum(index) = decisions.inputs[input].ty else {
                unreachable!("an `is` test names an enum input")
            };
            let (member, _) = &decisions.enums[index].members[member];
            format!(
                "i.{} == {}",
                input_base(language, decisions.inputs[input].name),
                language.ident(Role::Member, member)
            )
        }
    }
}

/// A list literal of annotations, `[]` when there are none.
fn annotations(language: &dyn Language, notes: &Notes) -> io::Result<String> {
    if notes.is_empty() {
        return Ok("[]".to_owned());
    }
    let mut items = Vec::new();
    for (key, value) in notes {
        let mut literal = Vec::new();
        language.annotation(key, *value, &mut literal)?;
        items.push(String::from_utf8(literal).map_err(io::Error::other)?);
    }
    Ok(format!("[ {} ]", items.join(", ")))
}
