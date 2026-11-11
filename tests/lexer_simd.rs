#![cfg(target_arch = "x86_64")]

#[path = "../src/lexer/simd.rs"]
mod simd;

fn scalar_prefix(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .enumerate()
        .find_map(|(index, byte)| {
            (matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'@' | b':')
                || (*byte == b'-' && bytes.get(index + 1) == Some(&b'>')))
            .then_some(index)
        })
        .unwrap_or(bytes.len())
}

#[test]
fn simd_matches_scalar_at_every_vector_alignment_and_arrow_boundary() {
    for padding in 0..32 {
        for length in 0..96 {
            for ending in [" ", "\t", "\r\n", "\n", "@", ":", "->", "-", "-->", "é", ""] {
                let source = format!(
                    "{}{}{ending}{}",
                    " ".repeat(padding),
                    "é-".repeat(length),
                    "x".repeat(40)
                );
                let bytes = &source.as_bytes()[padding..];
                assert_eq!(
                    simd::word_prefix_len(bytes),
                    scalar_prefix(bytes),
                    "{padding} {length} {ending:?}"
                );
                // Exercise short loads and the exact end of the allocation too.
                for end in 0..bytes.len().min(40) {
                    assert_eq!(
                        simd::word_prefix_len(&bytes[..end]),
                        scalar_prefix(&bytes[..end])
                    );
                }
            }
        }
    }
}

#[test]
fn simd_matches_scalar_for_seeded_arbitrary_bytes() {
    let mut state = 42u64;
    for length in 0..512 {
        let bytes: Vec<_> = (0..length)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 32) as u8
            })
            .collect();
        for start in 0..bytes.len() {
            assert_eq!(
                simd::word_prefix_len(&bytes[start..]),
                scalar_prefix(&bytes[start..])
            );
        }
    }
}
