//! The shared pure front end. `Pure<S>` implements [`Frontend`] once for every pure language: a
//! language supplies a [`PureSyntax`] (its layout and the names it generates) over its
//! [`Language`] (its rules). The module plan, the file name, the collision check and the lowering
//! are written here, so a new language is one `Language`, one `PureSyntax`, and one target.

pub mod decision;
pub mod purescript;
#[cfg(test)]
mod tests;

use std::io::{self, Write};

use super::NameClash;
use super::lang::{Language, Namespace, Role};
use super::names::Names;
use super::{Frontend, Module};
use crate::graph::CompiledGraph;
use crate::kind::Kind;

use decision::{Decisions, lower};

/// One pure language's layout. Every literal and name goes through `language()`.
pub trait PureSyntax: Sync {
    /// What a syntax computes once per module from its decisions: the names it will emit. The
    /// collision check and `module` both read this one value.
    type Layout<'d>;

    /// The rules this syntax writes with. The extension, names and literals all come from here.
    fn language(&self) -> &'static dyn Language;

    /// Computes the layout of one module.
    fn layout<'d>(&self, decisions: &Decisions<'d>) -> Self::Layout<'d>;

    /// Every name `module` emits that does not come from a source name, with its role and the
    /// declaration that generates it. The collision check registers these before any source name,
    /// so a source name that clashes with one of them is the error.
    fn generated<'d>(
        &self,
        decisions: &Decisions<'d>,
        layout: &Self::Layout<'d>,
    ) -> Vec<Generated<'d>>;

    /// Names the module imports and uses, which a source name must not spell. A source name that
    /// clashes with one of them is the error, as with a generated name.
    fn imported(&self) -> &'static [(Role, &'static str)] {
        &[]
    }

    /// The layout of one module, written from its decisions and the layout computed for them.
    fn module<'d>(
        &self,
        decisions: &Decisions<'d>,
        layout: &Self::Layout<'d>,
        output: &mut dyn Write,
    ) -> io::Result<()>;
}

/// The pure front end for one language's syntax.
pub struct Pure<S: PureSyntax> {
    syntax: S,
}

impl<S: PureSyntax> Pure<S> {
    pub const fn new(syntax: S) -> Self {
        Self { syntax }
    }
}

impl<S: PureSyntax> Frontend for Pure<S> {
    fn modules(&self, _: &CompiledGraph) -> Vec<Module> {
        vec![Module::Index]
    }

    fn file_name(&self, graph: &CompiledGraph, _: Module) -> String {
        let language = self.syntax.language();
        let stem = language.ident(Role::Module, graph.text(graph.name));
        format!("{stem}.{}", language.extension())
    }

    fn emit_module(
        &self,
        graph: &CompiledGraph,
        _: Module,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        let decisions = lower(graph);
        let layout = self.syntax.layout(&decisions);
        check_names(&self.syntax, &decisions, &layout)?;
        self.syntax.module(&decisions, &layout, output)
    }

    /// A pure front end writes no runtime, so this is never called. See `runtime_file_name`.
    fn emit_runtime(&self, _: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }

    fn runtime_file_name(&self) -> Option<&'static str> {
        None
    }

    fn extension(&self) -> &'static str {
        self.syntax.language().extension()
    }

    fn kind(&self) -> Kind {
        Kind::Pure
    }

    /// Only the names: the layout is not rendered, so a check costs one pass over the names.
    fn check_graph(&self, graph: &CompiledGraph) -> io::Result<()> {
        let decisions = lower(graph);
        let layout = self.syntax.layout(&decisions);
        check_names(&self.syntax, &decisions, &layout)
    }
}

/// A name the layout emits that no source spells: its role, its spelling, and the declaration that
/// generates it. A name every module declares, such as `Input`, has no `source`.
pub struct Generated<'d> {
    pub role: Role,
    pub name: String,
    pub source: Option<Source<'d>>,
}

/// The source declaration that a generated name comes from.
#[derive(Clone, Copy)]
pub struct Source<'d> {
    /// The declaration's name as written, such as `home-east`.
    pub name: &'d str,
    /// What kind of declaration it is, such as `input`, for a diagnostic.
    pub kind: &'static str,
    pub offset: usize,
}

/// Where a name came from. A clash with a generated or imported name is reported as one.
#[derive(Clone, Copy)]
enum Origin<'a> {
    Source,
    Generated(Option<Source<'a>>),
    Imported,
}

/// What the collision table keeps for a name: the name as spelled, the offset of the declaration
/// that holds it, and where it came from.
#[derive(Clone, Copy)]
struct Holder<'a> {
    name: &'a str,
    offset: usize,
    origin: Origin<'a>,
    /// The enum a member belongs to, so two members with one spelling name their enums.
    scope: Option<&'a str>,
}

/// Checks that no two names in a module spell the same identifier in the same namespace. The
/// generated names are registered first, so a source name that clashes with one of them is the
/// error, and the source names follow in source order. A clash is reported at the later
/// declaration, so its offset is that declaration's own.
fn check_names<'d, S: PureSyntax>(
    syntax: &S,
    decisions: &Decisions<'d>,
    layout: &S::Layout<'d>,
) -> io::Result<()> {
    let generated = syntax.generated(decisions, layout);
    let mut claims = Claims {
        names: Names::default(),
        language: syntax.language(),
    };
    for item in &generated {
        let holder = Holder {
            name: &item.name,
            offset: item.source.map_or(decisions.offset, |source| source.offset),
            origin: Origin::Generated(item.source),
            scope: None,
        };
        claims.claim_spelled(item.role, &item.name, holder, None)?;
    }
    for &(role, name) in syntax.imported() {
        let holder = Holder {
            name,
            offset: decisions.offset,
            origin: Origin::Imported,
            scope: None,
        };
        claims.claim_spelled(role, name, holder, None)?;
    }
    claims.claim(Role::Module, decisions.name, decisions.offset, None)?;
    for item in &decisions.enums {
        claims.claim(Role::Type, item.name, item.offset, None)?;
        for member in &item.members {
            claims.claim(Role::Member, member.name, member.offset, Some(item.name))?;
        }
    }
    for input in &decisions.inputs {
        claims.claim(Role::Field, input.name, input.offset, Some("Input"))?;
    }
    for function in &decisions.functions {
        claims.claim(Role::Function, function.name, function.offset, None)?;
    }
    Ok(())
}

fn clash(message: String, offset: usize) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, NameClash { message, offset })
}

/// The message for a clash where `holder` came second and `first` holds `spelled`.
fn clash_message(holder: Holder, first: Holder, spelled: &str) -> String {
    match (holder.origin, first.origin) {
        (Origin::Generated(Some(this)), Origin::Generated(Some(that))) => format!(
            "the {} `{}` and the {} `{}` both generate `{spelled}`; rename one of them",
            this.kind, this.name, that.kind, that.name
        ),
        (_, Origin::Generated(Some(from))) => format!(
            "`{}` clashes with the name `{}`, which the {} `{}` generates",
            holder.name, first.name, from.kind, from.name
        ),
        (_, Origin::Generated(None)) => format!(
            "`{}` clashes with the generated name `{}`",
            holder.name, first.name
        ),
        (_, Origin::Imported) => format!(
            "`{}` clashes with the imported name `{}`",
            holder.name, first.name
        ),
        (_, Origin::Source) => match (holder.scope, first.scope) {
            (Some(this), Some(that)) if this != that => format!(
                "`{}` of `{this}` and `{}` of `{that}` both become `{spelled}`",
                holder.name, first.name
            ),
            _ => format!(
                "`{}` and `{}` both become `{spelled}`",
                holder.name, first.name
            ),
        },
    }
}

/// The names claimed so far in one module.
struct Claims<'a> {
    names: Names<(Namespace, Option<&'a str>), Holder<'a>>,
    language: &'a dyn Language,
}

impl<'a> Claims<'a> {
    /// Spells source name `name` for `role` and records it. `owner` scopes an `Owner` name; a
    /// `Module` name ignores it. A spelling the language cannot use is refused.
    fn claim(
        &mut self,
        role: Role,
        name: &'a str,
        offset: usize,
        owner: Option<&'a str>,
    ) -> io::Result<()> {
        let spelled = self.language.ident(role, name);
        if !self.language.is_valid(role, &spelled) {
            let message = if spelled.is_empty() {
                format!("`{name}` has no letters or digits to make a name from")
            } else {
                format!("`{name}` becomes `{spelled}`, which cannot be used as this name")
            };
            return Err(clash(message, offset));
        }
        let holder = Holder {
            name,
            offset,
            origin: Origin::Source,
            scope: owner,
        };
        self.claim_spelled(role, &spelled, holder, owner)
    }

    /// Records an already spelled name. A clash is an error at the holder's offset, naming the
    /// first holder.
    fn claim_spelled(
        &mut self,
        role: Role,
        spelled: &str,
        holder: Holder<'a>,
        owner: Option<&'a str>,
    ) -> io::Result<()> {
        let namespace = self.language.namespace(role);
        let key = match namespace {
            Namespace::Owner => (namespace, owner),
            Namespace::Module(_) => (namespace, None),
        };
        self.names
            .insert(key, spelled.to_owned(), holder)
            .map_err(|first| clash(clash_message(holder, first, spelled), holder.offset))
    }
}
