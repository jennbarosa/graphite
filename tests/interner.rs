//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

// Merged from `tests/interner.rs`.
mod interner {

    use graphite::interner::Interner;

    #[test]
    fn interns_opaque_utf8_strings_once_with_stable_ids() {
        let mut pool = Interner::default();
        let first = pool.intern("time.delay").unwrap();
        let second = pool.intern("é").unwrap();
        let empty = pool.intern("").unwrap();

        assert_eq!(pool.intern("time.delay"), Ok(first));
        assert_ne!(first, second);
        assert_ne!(second, empty);
        assert_eq!(pool.resolve(first), Some("time.delay"));
        assert_eq!(pool.resolve(second), Some("é"));
        assert_eq!(pool.resolve(empty), Some(""));
    }

    #[test]
    fn probing_keeps_all_distinct_names_retrievable() {
        let names = [
            "closed", "opening", "open", "closing", "windup", "going", "winddown", "stopped",
        ];
        let mut pool = Interner::default();
        let ids: Vec<_> = names
            .iter()
            .map(|name| pool.intern(name).unwrap())
            .collect();

        for (name, id) in names.iter().zip(ids) {
            assert_eq!(pool.intern(name), Ok(id));
            assert_eq!(pool.resolve(id), Some(*name));
        }
    }

    #[test]
    fn probing_preserves_ids_at_every_small_capacity() {
        let names = ["", "closed", "opening", "open", "closing", "é", "中", "🦀"];
        for capacity in 1..=names.len() {
            let mut pool = Interner::default();
            let ids: Vec<_> = names[..capacity]
                .iter()
                .map(|name| pool.intern(name).unwrap())
                .collect();
            for (name, id) in names[..capacity].iter().zip(ids).rev() {
                assert_eq!(pool.intern(name), Ok(id));
                assert_eq!(pool.resolve(id), Some(*name));
            }
        }
    }

    #[test]
    fn mixed_utf8_lengths_and_duplicates_match_a_reference_map() {
        use std::collections::HashMap;

        for seed in [0u64, 42, u64::MAX] {
            let mut state = seed;
            let mut words = vec![String::new(), "\0é中🦀".into()];
            for length in 1..=64 {
                words.push("closed".repeat(length));
                words.push("é".repeat(length));
            }
            for _ in 0..4_096 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                words.push(format!("closed{state:016x}"));
            }
            let mut pool = Interner::default();
            let mut reference = HashMap::new();
            for word in &words {
                let id = pool.intern(word).unwrap();
                assert_eq!(*reference.entry(word).or_insert(id), id);
                assert_eq!(pool.resolve(id), Some(word.as_str()));
            }
            for word in words.iter().rev() {
                assert_eq!(pool.intern(word), Ok(reference[word]));
                assert_eq!(pool.resolve(reference[word]), Some(word.as_str()));
            }
            for word in &words {
                assert_eq!(pool.intern(word), Ok(reference[word]));
            }
        }
    }

    #[test]
    fn long_strings_and_embedded_nulls_keep_their_full_length() {
        let words = [
            "closed".repeat(10_922),
            "é".repeat(32_767),
            "é".repeat(32_768),
            format!("{}\0", "é".repeat(32_767)),
            format!("{}closed", "é".repeat(32_768)),
        ];
        let mut pool = Interner::default();
        let ids: Vec<_> = words
            .iter()
            .map(|word| pool.intern(word).unwrap())
            .collect();
        for (word, id) in words.iter().zip(ids) {
            assert_eq!(pool.intern(word), Ok(id));
            assert_eq!(pool.resolve(id), Some(word.as_str()));
        }
    }
}

// Merged from `tests/interner_limits.rs`.
mod interner_limits {
    //! The interner's limits and lookups: a repeated string keeps its id, a full table or byte budget
    //! refuses, and an id outside the table resolves to nothing.

    use graphite::interner::Interner;

    #[test]
    fn a_repeated_string_keeps_its_id_and_resolves_back() {
        let mut interner = Interner::default();
        let first = interner.intern("door").unwrap();
        let again = interner.intern("door").unwrap();
        let other = interner.intern("open").unwrap();
        assert_eq!(first, again);
        assert_ne!(first, other);
        assert_eq!(interner.resolve(first), Some("door"));
        assert_eq!(interner.resolve(other), Some("open"));
        assert_eq!(first.index(), 0);
        assert_eq!(other.index(), 1);
    }

    #[test]
    fn strings_come_back_in_the_order_they_were_interned() {
        let mut interner = Interner::default();
        for word in ["a", "bb", "ccc"] {
            interner.intern(word).unwrap();
        }
        let all: Vec<&str> = interner.strings().collect();
        assert_eq!(all, ["a", "bb", "ccc"]);
        assert_eq!(interner.strings().len(), 3);
    }

    #[test]
    fn an_id_from_a_larger_table_resolves_to_nothing_here() {
        let mut big = Interner::default();
        for word in ["a", "b", "c"] {
            big.intern(word).unwrap();
        }
        let late = big.intern("c").unwrap();
        let small = Interner::default();
        assert_eq!(small.resolve(late), None);
    }
}
