/// Seeded synthetic compiler identifiers; generation stays outside timed regions.
pub fn names(count: usize, seed: u64) -> Vec<String> {
    let mut state = seed;
    (0..count)
        .map(|index| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            format!("node_{index}_{state:016x}")
        })
        .collect()
}
