//! The shared pure front end. `Pure<S>` implements [`Frontend`] once for every pure language: a
//! language supplies a [`PureSyntax`] (its layout and the names it generates) over its
//! [`Language`] (its rules). The module plan, the file name, the collision check and the lowering
//! are written here, so a new language is one `Language`, one `PureSyntax`, and one target.

pub mod decision;
#[cfg(test)]
mod tests;

use std::io::{self, Write};

use super::lang::{Language, Namespace, Role};
use super::names::Names;
use super::{Frontend, Module};
use crate::graph::CompiledGraph;
use crate::kind::Kind;

use decision::{Decisions, lower};

/// One pure language's layout. Every literal and name goes through `language()`.
pub trait PureSyntax: Sync {
    /// The rules this syntax writes with. The extension, names and literals all come from here.
    fn language(&self) -> &'static dyn Language;

    /// Every name `module` emits that does not come from a source name, with its role. The
    /// collision check registers these before any source name, so a source name that clashes
    /// with one of them is the error.
    fn generated(&self, decisions: &Decisions) -> Vec<(Role, String)>;

    /// The layout of one module.
    fn module(&self, decisions: &Decisions, output: &mut dyn Write) -> io::Result<()>;
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
        check_names(&self.syntax, &decisions)?;
        self.syntax.module(&decisions, output)
    }

    /// A pure front end writes no runtime, so this is never called. See `runtime_file_name_opt`.
    fn emit_runtime(&self, _: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }

    fn runtime_file_name(&self) -> &'static str {
        ""
    }

    fn extension(&self) -> &'static str {
        self.syntax.language().extension()
    }

    fn kind(&self) -> Kind {
        Kind::Pure
    }

    fn runtime_file_name_opt(&self) -> Option<&'static str> {
        None
    }
}

/// Checks that no two names in a module spell the same identifier in the same namespace. The
/// generated names are registered first, so a source name that clashes with one of them is the
/// error, and the source names follow in source order.
fn check_names(syntax: &dyn PureSyntax, decisions: &Decisions) -> io::Result<()> {
    let generated = syntax.generated(decisions);
    let mut claims = Claims {
        names: Names::default(),
        language: syntax.language(),
        generated: &generated,
    };
    for (role, name) in &generated {
        claims.claim(*role, name, name, None)?;
    }
    claims.claim(Role::Module, decisions.name, decisions.name, None)?;
    for item in &decisions.enums {
        claims.claim(Role::Type, item.name, item.name, None)?;
        for (member, _) in &item.members {
            claims.claim(Role::Member, member, member, Some(item.name))?;
        }
    }
    for input in &decisions.inputs {
        claims.claim(Role::Field, input.name, input.name, Some("Input"))?;
    }
    for function in &decisions.functions {
        claims.claim(Role::Function, function.name, function.name, None)?;
    }
    Ok(())
}

/// The names claimed so far in one module, and the generated names they are checked against.
struct Claims<'a> {
    names: Names<'a, (Namespace, Option<&'a str>)>,
    language: &'a dyn Language,
    generated: &'a [(Role, String)],
}

impl<'a> Claims<'a> {
    /// Spells `name` for `role` and records it. `owner` scopes an `Owner` name; a `Module` name
    /// ignores it. A clash names the first holder, and says so when that holder is generated.
    fn claim(
        &mut self,
        role: Role,
        name: &'a str,
        holder: &'a str,
        owner: Option<&'a str>,
    ) -> io::Result<()> {
        let namespace = self.language.namespace(role);
        let key = match namespace {
            Namespace::Owner => (namespace, owner),
            Namespace::Module(_) => (namespace, None),
        };
        let spelled = self.language.ident(role, name);
        self.names
            .insert(key, spelled.clone(), holder)
            .map_err(|first| {
                let generated = self.generated.iter().any(|(_, text)| text == first);
                let message = if generated {
                    format!("`{holder}` clashes with the generated name `{first}`")
                } else {
                    format!("`{holder}` and `{first}` both become `{spelled}`")
                };
                io::Error::new(io::ErrorKind::InvalidData, message)
            })
    }
}
