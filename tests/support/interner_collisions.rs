use super::{Interner, InternerFull, StringId};
use std::hash::BuildHasher;

#[test]
fn equal_hashes_still_require_equal_string_bytes() {
    let mut pool = Interner::with_capacity(2, 32).unwrap();
    let closed = pool.intern("closed").unwrap();
    let hash = pool.hasher.hash_one("opening");

    // Force a full hash collision without relying on a particular hashing algorithm.
    pool.slots.clear();
    pool.slots.insert_unique(hash, closed, |_| hash);
    assert_eq!(pool.find("opening"), None);
    let opening = pool.intern("opening").unwrap();
    assert_ne!(opening, closed);
    assert_eq!(pool.find("opening"), Some(opening));
    assert_eq!(pool.resolve(closed), Some("closed"));
    assert_eq!(pool.resolve(opening), Some("opening"));
    assert_eq!(pool.intern("opening"), Ok(opening));
    assert_eq!(pool.intern("open"), Err(InternerFull));
}

#[test]
fn out_of_range_symbols_do_not_index_the_offset_array() {
    let mut pool = Interner::with_capacity(1, 6).unwrap();
    assert_eq!(pool.resolve(StringId::from_index(0)), None);
    pool.intern("closed").unwrap();
    assert_eq!(pool.resolve(StringId::from_index(1)), None);
    assert_eq!(pool.resolve(StringId::from_index(u32::MAX - 1)), None);
}
