use std::cell::RefCell;
use std::io::{self, Cursor, Write};
use std::rc::Rc;

use graphite::parser::ParseError;

#[allow(dead_code)]
#[path = "../benches/support/profile.rs"]
mod profile;

struct Control(Rc<RefCell<Vec<&'static str>>>);

impl Write for Control {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().push(match bytes {
            b"enable\n" => "enable",
            b"disable\n" => "disable",
            _ => panic!("unexpected perf command"),
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Output(Rc<RefCell<Vec<&'static str>>>);

impl Drop for Output {
    fn drop(&mut self) {
        self.0.borrow_mut().push("drop");
    }
}

#[test]
fn nul_terminated_perf_acknowledgements_do_not_shift_the_next_command() {
    let mut control = Vec::new();
    profile::run(
        &mut control,
        &mut Cursor::new(b"ack\n\0ack\n\0ack\n\0ack\n\0"),
        2,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(control, b"enable\ndisable\nenable\ndisable\n");
}

#[test]
fn sampling_wraps_parse_and_disables_before_output_destruction() {
    let events = Rc::new(RefCell::new(Vec::new()));
    profile::run(
        &mut Control(events.clone()),
        &mut Cursor::new(b"ack\nack\nack\nack\n"),
        2,
        || {
            events.borrow_mut().push("parse");
            Ok(Output(events.clone()))
        },
    )
    .unwrap();
    assert_eq!(
        *events.borrow(),
        [
            "enable", "parse", "disable", "drop", "enable", "parse", "disable", "drop"
        ]
    );
}

#[test]
fn a_failed_parse_disables_sampling_before_returning_the_error() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let error = profile::run::<()>(
        &mut Control(events.clone()),
        &mut Cursor::new(b"ack\nack\n"),
        2,
        || {
            events.borrow_mut().push("parse");
            Err(ParseError {
                message: "invalid name",
                offset: 0,
            })
        },
    )
    .unwrap_err();
    assert_eq!(*events.borrow(), ["enable", "parse", "disable"]);
    assert!(error.to_string().contains("invalid name"));
}

#[test]
fn missing_or_invalid_acknowledgements_prevent_parsing() {
    for response in [b"".as_slice(), b"invalid\n"] {
        let mut control = Vec::new();
        let error = profile::run(&mut control, &mut Cursor::new(response), 1, || {
            panic!("must not parse before perf acknowledges enable");
            #[allow(unreachable_code)]
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData
        ));
        assert_eq!(control, b"enable\n");
    }
}

#[test]
fn zero_iterations_is_rejected_without_enabling_sampling() {
    let mut control = Vec::new();
    assert!(profile::run(&mut control, &mut Cursor::new(b""), 0, || Ok(())).is_err());
    assert!(control.is_empty());
}
