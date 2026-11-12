//! The allowlist `--allow` reads: a TOML file that lists the values each annotation key may
//! carry, and the domain node flags the graphs may use.
//!
//!     node_flags = ["retry"]
//!
//!     [annotations]
//!     task = ["write", "lint", "deploy"]
//!
//! Annotations whose key is not listed are unrestricted. A listed key must carry one of its
//! values, and a bare `@key` with no value is refused.

use std::collections::BTreeMap;
use std::path::Path;

use graphite::graph::{Annotation, CompiledGraph, Span, index};

pub struct Allowlist {
    values: BTreeMap<String, Vec<String>>,
    node_flags: Vec<String>,
}

/// The strings in a TOML array, or `None` when the value is not an array of strings.
fn string_list(value: &toml::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

impl Allowlist {
    /// An allowlist that restricts nothing: no annotation key is limited and no node flag is added.
    pub fn unrestricted() -> Self {
        Self {
            values: BTreeMap::new(),
            node_flags: Vec::new(),
        }
    }

    /// This allowlist with the allowed values for `key` replaced.
    pub fn restrict(mut self, key: &str, mut values: Vec<String>) -> Self {
        values.sort();
        values.dedup();
        self.values.insert(key.to_string(), values);
        self
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let table: toml::Table =
            toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        for section in table.keys() {
            if section != "annotations" && section != "node_flags" {
                return Err(format!(
                    "{}: unknown key [{section}]; the allowlist has [annotations] and node_flags",
                    path.display()
                ));
            }
        }
        let node_flags = match table.get("node_flags") {
            None => Vec::new(),
            Some(value) => string_list(value).ok_or_else(|| {
                format!("{}: node_flags must be an array of strings", path.display())
            })?,
        };
        // An allowlist may hold only node flags, which leaves every annotation unrestricted.
        let annotations = match table.get("annotations") {
            None => toml::Table::new(),
            Some(value) => value
                .as_table()
                .cloned()
                .ok_or_else(|| format!("{}: [annotations] must be a table", path.display()))?,
        };
        let mut values = BTreeMap::new();
        for (key, list) in &annotations {
            let allowed = string_list(list).ok_or_else(|| {
                format!(
                    "{}: [annotations] {key} must be an array of strings",
                    path.display()
                )
            })?;
            let mut allowed = allowed;
            allowed.sort();
            allowed.dedup();
            values.insert(key.clone(), allowed);
        }
        Ok(Self { values, node_flags })
    }

    /// The domain node flags the allowlist declares, beyond the core `loop` and `end`.
    pub fn node_flags(&self) -> Vec<&str> {
        self.node_flags.iter().map(String::as_str).collect()
    }

    /// The first annotation outside the allowlist, as an error naming where it sits.
    pub fn check(&self, graphs: &[CompiledGraph]) -> Result<(), String> {
        for graph in graphs {
            let graph_name = graph.text(graph.name);
            for node in &graph.nodes {
                let place = || format!("graph {graph_name}, node {}", graph.text(node.name));
                self.check_span(graph, &node.annotations, &place)?;
            }
            for choice in &graph.choices {
                let choice_name = graph.text(choice.name);
                for arm in &graph.arms[index(&choice.arms)] {
                    let place = || format!("graph {graph_name}, choice {choice_name}");
                    self.check_span(graph, &arm.annotations, &place)?;
                }
            }
            for edge in &graph.edges {
                let place = || {
                    format!(
                        "graph {graph_name}, edge {} -> {}",
                        graph.text(graph.nodes[edge.source as usize].name),
                        graph.text(graph.nodes[edge.target as usize].name),
                    )
                };
                self.check_span(graph, &edge.annotations, &place)?;
            }
        }
        Ok(())
    }

    fn check_span(
        &self,
        graph: &CompiledGraph,
        span: &Span,
        place: &dyn Fn() -> String,
    ) -> Result<(), String> {
        for Annotation { key, value } in &graph.annotations[index(span)] {
            let key = graph.text(*key);
            let Some(allowed) = self.values.get(key) else {
                continue;
            };
            match value {
                None => {
                    return Err(format!(
                        "{}: @{key} needs a value; allowed: {}",
                        place(),
                        allowed.join(", ")
                    ));
                }
                Some(value) => {
                    let value = graph.text(*value);
                    // The values are kept sorted, so membership is a binary search.
                    if allowed
                        .binary_search_by(|allowed| allowed.as_str().cmp(value))
                        .is_err()
                    {
                        return Err(format!(
                            "{}: @{key}:{value} is not allowed; allowed: {}",
                            place(),
                            allowed.join(", ")
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
