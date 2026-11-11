//! Perf control for the benchmark only; production parsing has no instrumentation.

use std::fs::{File, OpenOptions};
use std::hint::black_box;
use std::io::{self, BufRead, BufReader, Write};

use graphite::graph::CompiledGraph;
use graphite::parser::{Capacity, ParseError, Schema, parse};

pub fn from_environment(
    source: &str,
    schema: &Schema<'_>,
    capacity: &[Capacity],
) -> io::Result<bool> {
    let Some(control_path) = std::env::var_os("GRAPHITE_PROFILE_CONTROL") else {
        return Ok(false);
    };
    let ack_path = std::env::var_os("GRAPHITE_PROFILE_ACK").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing perf acknowledgement pipe",
        )
    })?;
    let iterations = std::env::var("GRAPHITE_PROFILE_ITERATIONS")
        .map_err(io::Error::other)?
        .parse::<usize>()
        .map_err(io::Error::other)?;
    let mut control = OpenOptions::new().write(true).open(control_path)?;
    let mut acknowledgements = BufReader::new(File::open(ack_path)?);
    run(&mut control, &mut acknowledgements, iterations, || {
        profile_parse(black_box(source), black_box(schema), black_box(capacity))
    })?;
    eprintln!("Isolated parse profile: {iterations} calls; output drop excluded");
    Ok(true)
}

// Preserve a named root and its frame in profile builds, without changing the library.
#[inline(never)]
pub fn profile_parse(
    source: &str,
    schema: &Schema<'_>,
    capacity: &[Capacity],
) -> Result<Vec<CompiledGraph>, ParseError> {
    black_box(parse(source, schema, capacity))
}

pub fn run<T>(
    control: &mut impl Write,
    acknowledgements: &mut impl BufRead,
    iterations: usize,
    mut parse: impl FnMut() -> Result<T, ParseError>,
) -> io::Result<()> {
    if iterations == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "iterations must be positive",
        ));
    }
    let mut response = String::new();
    for _ in 0..iterations {
        command(control, acknowledgements, &mut response, b"enable\n")?;
        let output = black_box(parse());
        command(control, acknowledgements, &mut response, b"disable\n")?;
        drop(output.map_err(io::Error::other)?);
    }
    Ok(())
}

fn command(
    control: &mut impl Write,
    acknowledgements: &mut impl BufRead,
    response: &mut String,
    command: &[u8],
) -> io::Result<()> {
    control.write_all(command)?;
    control.flush()?;
    response.clear();
    if acknowledgements.read_line(response)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "perf control pipe closed",
        ));
    }
    // Some perf versions include a NUL after the preceding acknowledgement's newline.
    if response.trim_start_matches('\0') != "ack\n" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid perf acknowledgement: {response:?}"),
        ));
    }
    Ok(())
}
