use oxidase::{transpile, Allocator, SourceType, String};

#[track_caller]
pub fn check_transpile(source: &str, expected_out: &str) {
    let allocator = Allocator::default();
    let mut source = String::from_str_in(source, &allocator);
    let ret = transpile(
        &allocator,
            SourceType::ts(),
            &mut source,
    );
    // assert!(
    //     ret.parser_errors.is_empty(),
    //     "Transpile returned errors: {:?}",
    //     ret.parser_errors
    // );
    assert_eq!(source.as_str(), expected_out);
}
pub fn check_transpile_identical(source: &str) {
    check_transpile(source, source);
}
