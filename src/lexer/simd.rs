use std::arch::x86_64::*;

use super::WORD_BREAK;

/// SSE2 is guaranteed on x86-64. Stop at ASCII boundaries without decoding UTF-8.
#[inline]
pub(super) fn word_prefix_len(bytes: &[u8]) -> usize {
    let mut position = 0;
    while bytes.len() - position >= 16 {
        // SAFETY: x86-64 has SSE2; the length check keeps the unaligned load in bounds.
        let mask = unsafe {
            let block = _mm_loadu_si128(bytes.as_ptr().add(position).cast());
            let mut breaks = _mm_cmpeq_epi8(block, _mm_set1_epi8(b' ' as i8));
            for byte in *b"\t\n\r@:-" {
                breaks = _mm_or_si128(breaks, _mm_cmpeq_epi8(block, _mm_set1_epi8(byte as i8)));
            }
            _mm_movemask_epi8(breaks) as u32
        };
        if mask == 0 {
            position += 16;
            continue;
        }
        position += mask.trailing_zeros() as usize;
        if bytes[position] != b'-' || bytes.get(position + 1) == Some(&b'>') {
            return position;
        }
        position += 1;
    }
    while let Some(&byte) = bytes.get(position) {
        if WORD_BREAK[byte as usize] || (byte == b'-' && bytes.get(position + 1) == Some(&b'>')) {
            break;
        }
        position += 1;
    }
    position
}
