use std::hash::BuildHasher;
use std::num::NonZeroU32;
use std::ops::Range;

use foldhash::fast::FixedState;
use hashbrown::HashTable;
use hashbrown::hash_table::Entry;

/// Stable within the interner that issued it; IDs must not cross interners.
/// Stores index + 1 so `Option<StringId>` takes four bytes, not eight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringId(NonZeroU32);

impl StringId {
    /// Zero-based index in the issuing interner's insertion-order string table.
    pub fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    pub(crate) fn from_index(index: u32) -> Self {
        Self(NonZeroU32::new(index + 1).unwrap())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InternerFull;

impl std::fmt::Display for InternerFull {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("interner capacity exhausted or unavailable")
    }
}

impl std::error::Error for InternerFull {}

/// Fixed-capacity UTF-8 storage with SwissTable lookup. The hash is unkeyed;
/// callers must only intern trusted input.
pub struct Interner {
    bytes: String,
    // Append-only storage needs only end offsets; the preceding end is the start.
    ends: Vec<u32>,
    slots: HashTable<StringId>,
    max_strings: usize,
    max_bytes: usize,
    hasher: FixedState,
}

impl Interner {
    /// Allocates storage once. Returns `InternerFull` if the capacity cannot be represented
    /// or the allocator refuses the memory.
    pub fn with_capacity(strings: usize, bytes: usize) -> Result<Self, InternerFull> {
        if u32::try_from(strings).is_err() || u32::try_from(bytes).is_err() {
            return Err(InternerFull);
        }

        let mut byte_buf = String::new();
        byte_buf
            .try_reserve_exact(bytes)
            .map_err(|_| InternerFull)?;
        let mut ends = Vec::new();
        ends.try_reserve_exact(strings).map_err(|_| InternerFull)?;
        let mut slots = HashTable::new();
        // Reserve while empty; the capacity guard below prevents later growth.
        slots
            .try_reserve(strings, |_| 0)
            .map_err(|_| InternerFull)?;

        Ok(Self {
            bytes: byte_buf,
            ends,
            slots,
            max_strings: strings,
            max_bytes: bytes,
            hasher: FixedState::default(),
        })
    }

    /// Expected O(value.len()) hashing plus O(1) probing; never allocates.
    /// Capacity failures leave stored strings and IDs intact.
    pub fn intern(&mut self, value: &str) -> Result<StringId, InternerFull> {
        if self.ends.len() == self.max_strings {
            return self.find(value).ok_or(InternerFull);
        }
        let hash = self.hasher.hash_one(value);
        let bytes = &mut self.bytes;
        let ends = &mut self.ends;
        let hasher = &self.hasher;
        match self.slots.entry(
            hash,
            |id| matches(bytes.as_bytes(), ends, *id, value.as_bytes()),
            |id| hasher.hash_one(resolve_from(bytes, ends, *id).unwrap()),
        ) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                if value.len() > self.max_bytes - bytes.len() {
                    return Err(InternerFull);
                }
                let id = StringId(NonZeroU32::new(ends.len() as u32 + 1).ok_or(InternerFull)?);
                bytes.push_str(value);
                ends.push(bytes.len() as u32);
                entry.insert(id);
                Ok(id)
            }
        }
    }

    /// Resolve a reference without inserting it or consuming storage capacity.
    pub(crate) fn find(&self, value: &str) -> Option<StringId> {
        self.slots
            .find(self.hasher.hash_one(value), |id| {
                matches(self.bytes.as_bytes(), &self.ends, *id, value.as_bytes())
            })
            .copied()
    }

    /// O(1) lookup into UTF-8 storage populated only through whole strings.
    pub fn resolve(&self, id: StringId) -> Option<&str> {
        resolve_from(&self.bytes, &self.ends, id)
    }

    /// Borrow strings in insertion order without allocating or exposing storage.
    pub fn strings(&self) -> impl ExactSizeIterator<Item = &str> {
        let mut start = 0;
        self.ends.iter().map(move |&end| {
            let value = &self.bytes[start..end as usize];
            start = end as usize;
            value
        })
    }
}

/// The byte range of the string at `index`. The caller checks that `index` is in range.
fn span(ends: &[u32], index: usize) -> Range<usize> {
    let start = if index == 0 {
        0
    } else {
        ends[index - 1] as usize
    };
    start..ends[index] as usize
}

fn matches(bytes: &[u8], ends: &[u32], id: StringId, value: &[u8]) -> bool {
    &bytes[span(ends, id.index())] == value
}

fn resolve_from<'a>(bytes: &'a str, ends: &[u32], id: StringId) -> Option<&'a str> {
    if id.index() >= ends.len() {
        return None;
    }
    bytes.get(span(ends, id.index()))
}

#[cfg(test)]
#[path = "../tests/support/interner_collisions.rs"]
mod tests;
