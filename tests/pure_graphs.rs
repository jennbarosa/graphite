//! Pure graphs, end to end through the `graphite` binary: each sample is compiled to PureScript,
//! the decision each generated choice makes is checked for every input, and the generated modules
//! are built by `purs` (through spago) in strict mode, so any warning fails the test.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;
#[allow(dead_code)]
#[path = "support/tools.rs"]
mod tools;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cli::{graphite, manifest, scratch, stderr, stdout};

/// The inputs of one decision: each input's name, and its value (`true`, `false`, or a member).
type Input = HashMap<String, String>;

/// The samples: the bundled example, and two fixtures for the cases the example leaves out.
const SAMPLES: [(&str, &str); 3] = [
    ("combat", "examples/combat.pure.graphite"),
    ("lamp_gate", "tests/fixtures/pure/lamp_gate.pure.graphite"),
    ("notes", "tests/fixtures/pure/notes.pure.graphite"),
];

fn flag(input: &Input, name: &str) -> bool {
    input[name] == "true"
}

/// A decision: a function from an input to the tag its generated choice returns.
type Decision = fn(&Input) -> &'static str;

/// Each generated choice, with the decision the source file means for it. The expected function is
/// written from the source's arms, in order: the first arm whose `when` holds wins, else the last.
fn expectations() -> Vec<(&'static str, &'static str, Decision)> {
    vec![
        ("Combat", "tactic", |i| {
            if flag(i, "lowHealth") {
                "Flee"
            } else if flag(i, "homeEast") {
                "Hit"
            } else {
                "Hold"
            }
        }),
        ("Combat", "facing", |i| {
            if i["dir"] == "East" { "East" } else { "North" }
        }),
        (
            "Lamp",
            "light",
            |i| if flag(i, "power") { "On" } else { "Off" },
        ),
        (
            "Gate",
            "lock",
            |i| if flag(i, "key") { "Open" } else { "Shut" },
        ),
        ("Notes", "alert", |i| {
            if flag(i, "urgent") { "High" } else { "Low" }
        }),
    ]
}

/// A generated choice, read back from its module's text: the input's fields and their values, the
/// tag of each arm value, and the guarded arms of the dispatch function, in order.
struct Choice {
    domains: Vec<(String, Vec<String>)>,
    tags: HashMap<String, String>,
    arms: Vec<(String, String)>,
}

impl Choice {
    /// Reads the choice `base` (its spelled name, such as `tactic`) from a generated module.
    fn read(text: &str, base: &str) -> Self {
        let fields = text
            .lines()
            .find_map(|line| line.strip_prefix("type Input = { "))
            .and_then(|line| line.strip_suffix(" }"))
            .expect("the module declares its Input record");
        let domains = fields
            .split(", ")
            .map(|field| {
                let (name, ty) = field.split_once(" :: ").expect("a field has a type");
                let domain = if ty == "Boolean" {
                    vec!["true".to_owned(), "false".to_owned()]
                } else {
                    members(text, ty)
                };
                (name.to_owned(), domain)
            })
            .collect();
        let tags = text
            .lines()
            .filter_map(|line| {
                let (name, rest) = line.split_once(" = { tag: ")?;
                let tag = rest.split(',').next()?;
                Some((name.to_owned(), tag.to_owned()))
            })
            .collect();
        let dispatch = format!("{base}Arm i");
        let mut lines = text.lines().skip_while(|line| *line != dispatch).skip(1);
        let arms = lines
            .by_ref()
            .take_while(|line| line.starts_with("  | "))
            .map(|line| {
                let (guard, arm) = line[4..].split_once(" = ").expect("a guard has an arm");
                (guard.to_owned(), arm.to_owned())
            })
            .collect();
        Choice {
            domains,
            tags,
            arms,
        }
    }

    /// The tag the generated code returns for `input`: the first guard that holds picks its arm.
    fn decide(&self, input: &Input) -> &str {
        for (guard, arm) in &self.arms {
            let holds = match guard.as_str() {
                "otherwise" => true,
                _ => match guard.strip_prefix("i.").expect("a guard tests an input") {
                    test if test.contains(" == ") => {
                        let (name, member) = test.split_once(" == ").unwrap();
                        input[name] == member
                    }
                    name => flag(input, name),
                },
            };
            if holds {
                return &self.tags[arm];
            }
        }
        panic!("no guard holds for {input:?}")
    }

    /// Every combination of input values.
    fn inputs(&self) -> Vec<Input> {
        let mut all = vec![Input::new()];
        for (name, values) in &self.domains {
            all = all
                .iter()
                .flat_map(|partial| {
                    values.iter().map(move |value| {
                        let mut next = partial.clone();
                        next.insert(name.clone(), value.clone());
                        next
                    })
                })
                .collect();
        }
        all
    }
}

/// The members of the enum `ty`, from its `data` line.
fn members(text: &str, ty: &str) -> Vec<String> {
    let line = text
        .lines()
        .find(|line| line.starts_with(&format!("data {ty} = ")))
        .expect("the module declares the enum");
    line.split(" = ")
        .nth(1)
        .unwrap()
        .split(" | ")
        .map(str::to_owned)
        .collect()
}

/// Compiles `sample` into `out` with the binary, and returns the directory.
fn compile(sample: &str, out: &Path) {
    let output = graphite([
        "compile".as_ref(),
        manifest().join(sample).as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(output.status.success(), "{sample}: {}", stderr(&output));
}

#[test]
fn every_generated_decision_matches_the_source_for_every_input() {
    let mut checked = 0;
    for (name, sample) in SAMPLES {
        let out = scratch(&format!("pure-decisions-{name}"));
        compile(sample, &out);
        for (module, base, expected) in expectations() {
            if !sample_has(name, module) {
                continue;
            }
            let text = std::fs::read_to_string(out.join(format!("{module}.purs")))
                .unwrap_or_else(|error| panic!("{module}.purs: {error}"));
            let choice = Choice::read(&text, base);
            let inputs = choice.inputs();
            assert!(!inputs.is_empty());
            for input in inputs {
                assert_eq!(
                    choice.decide(&input),
                    expected(&input),
                    "{module}.{base} for {input:?}"
                );
                checked += 1;
            }
        }
    }
    // Each choice runs over every input of its module: Combat has 2 * 2 * 4 = 16 combinations, for
    // each of its two choices; Lamp and Gate have 4 and 2; Notes has 2.
    assert_eq!(checked, 16 * 2 + 4 + 2 + 2);
}

/// Whether the sample `name` declares the graph `module`, so its choices are checked.
fn sample_has(name: &str, module: &str) -> bool {
    match name {
        "combat" => module == "Combat",
        "lamp_gate" => matches!(module, "Lamp" | "Gate"),
        "notes" => module == "Notes",
        _ => false,
    }
}

#[test]
fn a_file_with_several_graphs_writes_one_module_per_graph_and_no_runtime() {
    let out = scratch("pure-several");
    compile("tests/fixtures/pure/lamp_gate.pure.graphite", &out);
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["Gate.purs", "Lamp.purs"]);
}

#[test]
fn compiling_a_pure_file_twice_gives_the_same_bytes() {
    let first = scratch("pure-repeat-a");
    let second = scratch("pure-repeat-b");
    compile("examples/combat.pure.graphite", &first);
    compile("examples/combat.pure.graphite", &second);
    assert_eq!(
        std::fs::read(first.join("Combat.purs")).unwrap(),
        std::fs::read(second.join("Combat.purs")).unwrap()
    );
}

#[test]
fn a_pure_file_reports_its_declarations_as_json() {
    let output = graphite([
        "inspect".as_ref(),
        "--json".as_ref(),
        manifest().join("examples/combat.pure.graphite").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains(r#""kind":"pure""#), "{text}");
    assert!(text.contains(r#""enum":"Tactic""#), "{text}");
}

/// Writes every sample's modules into `project/src/<sample>/`, so `spago` builds them together.
fn write_project(project: &Path) {
    let _ = std::fs::remove_dir_all(project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::copy(
        manifest().join("tests/purescript/spago.yaml"),
        project.join("spago.yaml"),
    )
    .unwrap();
    for (name, sample) in SAMPLES {
        compile(sample, &project.join("src").join(name));
    }
}

#[test]
fn the_generated_modules_build_with_purs_in_strict_mode() {
    let project: PathBuf = scratch("pure-project");
    write_project(&project);
    let output = tools::command("spago", "GRAPHITE_SPAGO")
        .current_dir(&project)
        .env("PATH", tools::path_with_tools())
        .args(["build", "--strict"])
        .output()
        .unwrap_or_else(|error| {
            panic!("cannot run spago: {error}; run scripts/setup-tools.sh or set GRAPHITE_SPAGO")
        });
    let log = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        output.status.success(),
        "spago build --strict failed:\n{log}"
    );
    assert!(log.contains("Build succeeded"), "{log}");
}
