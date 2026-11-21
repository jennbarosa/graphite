#[path = "../benches/support/workload.rs"]
mod workload;

#[test]
fn seed_reproduces_unique_names_and_interns_them() {
    let names = workload::names(8, 42);
    assert_eq!(names, workload::names(8, 42));
    assert_ne!(names, workload::names(8, 43));
    assert_eq!(names.len(), 8);
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(unique.len(), names.len());
    let mut pool = graphite::interner::Interner::default();
    for name in &names {
        let id = pool.intern(name).unwrap();
        assert_eq!(pool.resolve(id), Some(name.as_str()));
    }
}
