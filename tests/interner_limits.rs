//! The interner's limits and lookups: a repeated string keeps its id, a full table or byte budget
//! refuses, and an id outside the table resolves to nothing.

use graphite::interner::{Interner, InternerFull};

#[test]
fn a_repeated_string_keeps_its_id_and_resolves_back() {
    let mut interner = Interner::with_capacity(4, 64).unwrap();
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
    let mut interner = Interner::with_capacity(4, 64).unwrap();
    for word in ["a", "bb", "ccc"] {
        interner.intern(word).unwrap();
    }
    let all: Vec<&str> = interner.strings().collect();
    assert_eq!(all, ["a", "bb", "ccc"]);
    assert_eq!(interner.strings().len(), 3);
}

#[test]
fn a_full_string_table_refuses_a_new_string_but_not_an_old_one() {
    let mut interner = Interner::with_capacity(2, 64).unwrap();
    interner.intern("one").unwrap();
    interner.intern("two").unwrap();
    assert_eq!(interner.intern("three"), Err(InternerFull));
    // A string already held still has its id.
    assert!(interner.intern("two").is_ok());
}

#[test]
fn a_byte_budget_refuses_the_string_that_would_exceed_it() {
    let mut interner = Interner::with_capacity(8, 4).unwrap();
    interner.intern("abcd").unwrap();
    assert_eq!(interner.intern("e"), Err(InternerFull));
}

#[test]
fn a_capacity_that_cannot_be_reserved_is_refused() {
    assert!(Interner::with_capacity(usize::MAX, 0).is_err());
    assert!(Interner::with_capacity(0, usize::MAX).is_err());
}

#[test]
fn an_id_from_a_larger_table_resolves_to_nothing_here() {
    let mut big = Interner::with_capacity(8, 64).unwrap();
    for word in ["a", "b", "c"] {
        big.intern(word).unwrap();
    }
    let late = big.intern("c").unwrap();
    let small = Interner::with_capacity(8, 64).unwrap();
    assert_eq!(small.resolve(late), None);
}

#[test]
fn the_full_table_message_reads_as_a_limit() {
    assert_eq!(
        InternerFull.to_string(),
        "interner capacity exhausted or unavailable"
    );
    let _: &dyn std::error::Error = &InternerFull;
}
