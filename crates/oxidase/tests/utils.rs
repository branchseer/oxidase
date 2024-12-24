use oxidase::{transpile, Allocator, Source, SourceType, TranspileOptions};

#[track_caller]
pub fn check_transpile(source: &str, expected_out: &str) {
    let allocator = Allocator::default();
    let mut source = Source::Borrowed(source);
    let ret = transpile(
        &allocator,
        TranspileOptions {
            source_type: SourceType::ts(),
            prefer_blank_space: false,
        },
        &mut source,
    );
    assert!(
        ret.parser_errors.is_empty(),
        "Transpile returned errors: {:?}",
        ret.parser_errors
    );
    assert_eq!(source.as_str(), expected_out);
}
pub fn check_transpile_identical(source: &str) {
    check_transpile(source, source);
}
