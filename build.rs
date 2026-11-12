//! Compiles the bundled rule files with the bootstrap converter, so the library can load them
//! without a parser. The converter is an earlier build of `examples/bootstrap_dump.rs`; see
//! CONTRIBUTING.md for how to get one.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const RULES: [(&str, &str); 3] = [
    ("src/graphs/lex.graphite", "lex.rs"),
    ("src/graphs/grammar.graphite", "grammar.rs"),
    ("src/graphs/structure.graphite", "structure.rs"),
];

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let converter = env::var_os("GRAPHITE_BOOTSTRAP")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/bootstrap/bootstrap_dump"));
    println!("cargo:rerun-if-env-changed=GRAPHITE_BOOTSTRAP");
    for (rule, generated) in RULES {
        println!("cargo:rerun-if-changed={rule}");
        let status = Command::new(&converter)
            .arg(root.join(rule))
            .arg(out.join(generated))
            .status()
            .unwrap_or_else(|error| {
                panic!(
                    "cannot run the bootstrap converter {}: {error}. See CONTRIBUTING.md",
                    converter.display()
                )
            });
        assert!(status.success(), "the bootstrap converter failed on {rule}");
    }
}
