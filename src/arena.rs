/// Index for the next item, or `None` if the arena is full or the index would not fit in `u32`.
pub fn next_index(len: usize, capacity: usize) -> Option<u32> {
    if len >= capacity {
        return None;
    }
    u32::try_from(len).ok()
}

/// Fixed-capacity arena. Items are addressed by `u32` index.
pub struct Arena<T> {
    items: Vec<T>,
    capacity: usize,
}

/// Returned when `push` is called on a full arena.
#[derive(Debug, PartialEq, Eq)]
pub struct ArenaFull;

impl std::fmt::Display for ArenaFull {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("arena capacity exhausted or unavailable")
    }
}

impl std::error::Error for ArenaFull {}

impl<T> Arena<T> {
    /// Allocates storage once. Returns `ArenaFull` if the allocator refuses the memory.
    pub fn with_capacity(capacity: usize) -> Result<Self, ArenaFull> {
        let mut items = Vec::new();
        items.try_reserve_exact(capacity).map_err(|_| ArenaFull)?;
        Ok(Self { items, capacity })
    }

    /// O(1), no allocation: `Vec::push` stays within the preallocated capacity.
    pub fn push(&mut self, value: T) -> Result<u32, ArenaFull> {
        let index = next_index(self.items.len(), self.capacity).ok_or(ArenaFull)?;
        self.items.push(value);
        Ok(index)
    }

    pub fn get(&self, index: u32) -> Option<&T> {
        self.items.get(index as usize)
    }

    /// Empties the arena but keeps its allocation for reuse.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
