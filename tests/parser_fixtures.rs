use graphite::parser::Schema;

#[path = "support/parse.rs"]
mod fixture;
use fixture::parse;

#[path = "support/inspect.rs"]
mod inspect;

macro_rules! fixture {
    ($name:ident $(, $flag:literal)*) => {
        #[test]
        fn $name() {
            let source = include_str!(concat!("fixtures/parser/", stringify!($name), ".dsl"));
            let expected =
                include_str!(concat!("fixtures/parser/", stringify!($name), ".expected"));
            let schema = Schema { node_flags: &[$($flag),*] };
            let graphs = parse(source, &schema).unwrap();
            let actual = inspect::graphs(&graphs);
            println!("{actual}");
            assert_eq!(
                actual,
                expected,
                "{} compiled output differs",
                stringify!($name)
            );
        }
    };
}

fixture!(door);
fixture!(walk);
fixture!(interleaved);
fixture!(scopes);
fixture!(domain_flags, "looping");
