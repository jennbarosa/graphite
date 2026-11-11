//! `graphite init`: writes a small starter graph that `check` and `compile` accept as they are.

use std::io::Write;
use std::path::Path;

const STARTER: &str = "graph hello
  node idle  @state:idle
  node work  @state:work
  node done  end @state:done

  edge idle -> work when start
  edge work -> done
";

pub fn run(input: &Path) -> Result<(), String> {
    // `create_new` refuses to overwrite, and avoids a check-then-write race.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(input)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("{} already exists; choose another name", input.display())
            } else {
                format!("{}: {error}", input.display())
            }
        })?;
    file.write_all(STARTER.as_bytes())
        .map_err(|error| format!("{}: {error}", input.display()))?;
    println!("wrote {}", input.display());
    println!("next: graphite check {}", input.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::STARTER;

    #[test]
    fn the_starter_graph_has_the_expected_shape() {
        assert!(STARTER.starts_with("graph hello\n"));
        assert_eq!(STARTER.matches("node ").count(), 3);
    }
}
