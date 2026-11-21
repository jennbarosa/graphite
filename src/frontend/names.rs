//! Spelled-name collision checks. Two names collide when they spell the same identifier in the
//! same namespace. Each front end picks its namespaces; this table holds the first holder of each.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;

/// The first holder of each `(namespace, spelling)` pair, so a clash can name both sides.
pub struct Names<'a, K> {
    seen: HashMap<(K, String), &'a str>,
}

impl<K: Hash + Eq> Default for Names<'_, K> {
    fn default() -> Self {
        Self {
            seen: HashMap::new(),
        }
    }
}

impl<'a, K: Hash + Eq> Names<'a, K> {
    /// Records `spelling` in `namespace` for `holder`. On a clash, returns the first holder.
    pub fn insert(
        &mut self,
        namespace: K,
        spelling: String,
        holder: &'a str,
    ) -> Result<(), &'a str> {
        match self.seen.entry((namespace, spelling)) {
            Entry::Occupied(first) => Err(first.get()),
            Entry::Vacant(slot) => {
                slot.insert(holder);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Names;

    #[test]
    fn a_clash_in_one_namespace_returns_the_first_holder() {
        let mut names = Names::default();
        assert_eq!(names.insert("module", "Combat".into(), "combat"), Ok(()));
        assert_eq!(
            names.insert("module", "Combat".into(), "COMBAT"),
            Err("combat")
        );
    }

    #[test]
    fn the_same_spelling_in_different_namespaces_does_not_clash() {
        let mut names = Names::default();
        assert_eq!(names.insert("owner-a", "Flee".into(), "a"), Ok(()));
        assert_eq!(names.insert("owner-b", "Flee".into(), "b"), Ok(()));
    }

    #[test]
    fn a_failed_insert_keeps_the_first_holder() {
        let mut names = Names::default();
        names.insert("module", "x".into(), "first").unwrap();
        assert_eq!(names.insert("module", "x".into(), "second"), Err("first"));
        assert_eq!(names.insert("module", "x".into(), "third"), Err("first"));
    }
}
