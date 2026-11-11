use graphite::lexer::{LexError, Lexer, Token};

/// Experimental batch API: partition only at unindented, nonempty line starts.
/// Boundary discovery, thread startup, token buffers, and merging are included.
// shortcut: one top-level block stays serial; use indentation checkpoints if that must parallelize.
pub fn tokenize(source: &str, workers: usize) -> Result<Vec<Token<'_>>, LexError> {
    assert!(workers > 0, "workers must be positive");
    if workers == 1 {
        return Lexer::new(source).collect();
    }
    let mut starts = vec![0];
    let mut line_start = true;
    let stride = source.len().div_ceil(workers);
    for (offset, &byte) in source.as_bytes().iter().enumerate() {
        if line_start
            && !matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
            && offset > 0
            && starts.len() < workers
            && offset >= stride * starts.len()
        {
            starts.push(offset);
        }
        line_start = matches!(byte, b'\r' | b'\n');
    }
    if starts.len() == 1 {
        return Lexer::new(source).collect();
    }
    starts.push(source.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = starts
            .windows(2)
            .map(|bounds| {
                let start = bounds[0];
                let chunk = &source[start..bounds[1]];
                scope.spawn(move || {
                    Lexer::new(chunk)
                        .map(|result| {
                            result
                                .map(|mut token| {
                                    token.offset += start;
                                    token
                                })
                                .map_err(|mut error| {
                                    error.offset += start;
                                    error
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect();
        let mut tokens = Vec::new();
        // Join in source order to preserve the first error and deterministic output.
        for handle in handles {
            tokens.append(&mut handle.join().expect("lexer worker panicked")?);
        }
        Ok(tokens)
    })
}
