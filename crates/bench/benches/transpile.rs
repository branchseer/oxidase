mod strip_visit;

use std::{
    fs::read_to_string,
    hint::black_box,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use oxidase::{Allocator, SourceType, String};
use oxidase_tsc::Tsc;
use swc::try_with_handler;

fn oxidase(
    allocator: &Allocator,
    source: &mut String<'_>,
    allocate_ast: bool,
    allow_skip_ambient: bool,
) {
    let ret = if allocate_ast {
        oxidase::transpile_with_options(
            allocator,
            allocator,
            allow_skip_ambient,
            SourceType::ts(),
            source,
        )
    } else {
        oxidase::transpile_with_options(
            allocator,
            &oxidase::VoidAllocator::new(),
            allow_skip_ambient,
            SourceType::ts(),
            source,
        )
    };
    assert_eq!(ret.parser_panicked, false);
    assert!(ret.parser_errors.is_empty());
}

// fn oxidase_allocating_ast(allocator: &Allocator, source: &mut String<'_>) {
//     black_box({
//         let ret = oxidase::transpile_allocated(allocator, SourceType::ts(), source);
//         assert_eq!(ret.parser_panicked, false);
//         assert!(ret.parser_errors.is_empty());
//         source
//     });
// }
// TypeScript/src/compiler/checker.ts
pub fn criterion_benchmark(c: &mut Criterion) {
    let fixture_ecosystem_dir: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixture/ecosystem");
    let mut tsc = Tsc::new();

    let mut group = c.benchmark_group("Transpile");
    for (param_name, input_path) in [("parser.ts", "TypeScript/src/compiler/checker.ts")] {
        let mut allocator = oxidase::Allocator::default();
        let source = read_to_string(fixture_ecosystem_dir.join(input_path)).unwrap();
        dbg!(source.len());
        let source = tsc.process_ts(&source, true).unwrap().ts;
        dbg!(source.len());

        group.bench_with_input(
            BenchmarkId::new("oxidase", param_name),
            source.as_str(),
            |b, input| {
                b.iter_custom(|iters| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iters {
                        let mut source = String::from_str_in(input, &allocator);
                        let start = Instant::now();
                        oxidase(&allocator, &mut source, false, true);
                        black_box(source);
                        allocator.reset();
                        elapsed += start.elapsed();
                    }
                    elapsed
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("oxidase disable allow_skip_ambient", param_name),
            source.as_str(),
            |b, input| {
                b.iter_custom(|iters| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iters {
                        let mut source = String::from_str_in(input, &allocator);
                        let start = Instant::now();
                        oxidase(&allocator, &mut source, true, false);
                        black_box(source);
                        allocator.reset();
                        elapsed += start.elapsed();
                    }
                    elapsed
                });
            },
        );

        // group.bench_with_input(
        //     BenchmarkId::new("swc_fast_ts_strip", param_name),
        //     source.as_str(),
        //     |b, input| {
        //         b.iter_custom(|iters| {
        //             let mut elapsed = Duration::ZERO;

        //             swc_common::GLOBALS.set(&Default::default(), || {
        //                 for _ in 0..iters {
        //                     let cm = Arc::<swc_common::SourceMap>::default();
        //                     try_with_handler(cm.clone(), Default::default(), |handler| {
        //                         let cm: Arc<swc_common::SourceMap> = Default::default();
        //                         let input = input.to_owned();
        //                         let start = Instant::now();
        //                         let output = swc_fast_ts_strip::operate(
        //                             &cm,
        //                             handler,
        //                             input,
        //                             swc_fast_ts_strip::Options::default(),
        //                         )
        //                         .unwrap();
        //                         elapsed += start.elapsed();
        //                         drop(black_box(output));
        //                         Ok(())
        //                     })
        //                     .unwrap();
        //                 }
        //             });

        //             elapsed
        //         })
        //     },
        // );


        let mut allocator = Default::default();
        group.bench_with_input(
            BenchmarkId::new("oxc_parser", param_name),
            source.as_str(),
            |b, input| {
                b.iter_custom(|iters| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        let parser = oxc_parser::Parser::new(&allocator, input, oxc_span::SourceType::ts());
                        let mut source = String::from_str_in(input, &allocator);
                        let ret = parser.parse();
                        assert!(!ret.panicked);
                        assert!(ret.errors.is_empty());
                        let mut strip_visit = strip_visit::StripVisit::new(&allocator);
                        oxc_ast::visit::walk::walk_program(&mut strip_visit, &ret.program);
                        let patches = strip_visit.into_patches();
                        oxidase::apply_patches(&patches, &mut source);
                        drop((ret, patches));
                        black_box(source);
                        allocator.reset();
                        elapsed += start.elapsed();
                    }
                    elapsed
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
