#![no_main]

use graphite::lexer::Lexer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else { return };
    for item in Lexer::new(source) {
        match item {
            Ok(token) => assert!(token.offset <= source.len(), "token past input"),
            Err(error) => {
                let _ = error.diagnostic(source).to_string();
            }
        }
    }
});
