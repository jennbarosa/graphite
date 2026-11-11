use graphite::arena::{Arena, ArenaFull, next_index};

#[test]
fn zero_sized_items_obey_requested_capacity_after_clear() {
    let mut empty = Arena::<()>::with_capacity(0).unwrap();
    assert_eq!(empty.push(()), Err(ArenaFull));

    let mut arena = Arena::<()>::with_capacity(1).unwrap();
    assert_eq!(arena.push(()), Ok(0));
    assert_eq!(arena.push(()), Err(ArenaFull));
    arena.clear();
    assert_eq!(arena.push(()), Ok(0));
    assert_eq!(arena.push(()), Err(ArenaFull));
}

#[test]
fn arena_errors_work_with_standard_error_propagation() -> Result<(), Box<dyn std::error::Error>> {
    let mut arena = Arena::<u8>::with_capacity(0)?;
    let error = arena.push(1).unwrap_err();
    assert!(!error.to_string().is_empty());
    Ok(())
}

#[test]
fn clear_empties_arena() {
    let mut arena = Arena::with_capacity(2).unwrap();
    arena.push(1).unwrap();
    arena.push(2).unwrap();

    arena.clear();

    assert_eq!(arena.get(0), None);
}

#[test]
fn clear_lets_arena_be_filled_again_to_same_capacity() {
    let mut arena = Arena::with_capacity(2).unwrap();
    arena.push(1).unwrap();
    arena.push(2).unwrap();
    arena.clear();

    assert_eq!(arena.push(3), Ok(0));
    assert_eq!(arena.push(4), Ok(1));
    assert_eq!(arena.push(5), Err(ArenaFull));
}

#[test]
fn next_index_returns_none_when_index_would_exceed_u32() {
    assert_eq!(next_index(u32::MAX as usize, usize::MAX), Some(u32::MAX));
    assert_eq!(next_index(u32::MAX as usize + 1, usize::MAX), None);
}

#[test]
fn push_returns_sequential_indices_starting_at_zero() {
    let mut arena = Arena::with_capacity(3).unwrap();

    assert_eq!(arena.push("a"), Ok(0));
    assert_eq!(arena.push("b"), Ok(1));
    assert_eq!(arena.push("c"), Ok(2));
}

#[test]
fn full_arena_keeps_stored_items_after_rejected_push() {
    let mut arena = Arena::with_capacity(2).unwrap();
    assert_eq!(arena.push(1), Ok(0));
    assert_eq!(arena.push(2), Ok(1));

    assert_eq!(arena.push(3), Err(ArenaFull));
    assert_eq!(arena.get(0), Some(&1));
    assert_eq!(arena.get(1), Some(&2));
}
