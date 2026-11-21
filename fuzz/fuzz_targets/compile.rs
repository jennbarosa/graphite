#![no_main]

use graphite::parser::Schema;
use graphite::pipeline::compile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else { return };
    for pure in [false, true] {
        let schema = Schema { node_flags: &["flag"], pure };
        if let Err(error) = compile(source, &schema) {
            assert!(error.offset <= source.len(), "offset {} past input", error.offset);
            let _ = error.diagnostic(source).to_string();
        }
    }
});
