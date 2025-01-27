use std::path::Path;

use criterion::{measurement::WallTime, *};
use oxidase_bench::{remove_codegen, Benchee, OxcParser, Oxidase, SwcFastTsStrip};

fn bench<B: Benchee>(
    g: &mut BenchmarkGroup<'_, WallTime>,
    source: &str,
    is_source_without_codegen: bool,
) {
    let mut benchee = B::default();
    let id = BenchmarkId::new(
        B::NAME,
        if is_source_without_codegen {
            "without-codegen"
        } else {
            "original"
        },
    );
    g.bench_function(id, |b| {
        b.iter_batched(
            || source.to_string(),
            |mut source| benchee.run(&mut source),
            BatchSize::SmallInput,
        )
    });
}

fn transpile_benchmark(c: &mut Criterion) {
    let filenames = ["checker.ts", "render.ts"];
    for filename in filenames {
        let path = Path::new("files").join(filename);
        let source = std::fs::read_to_string(&path).unwrap();

        let mut g = c.benchmark_group(filename);

        for without_codegen in [false, true] {
            let source = if without_codegen {
                remove_codegen(&source)
            } else {
                source.clone()
            };

            bench::<Oxidase>(&mut g, &source, without_codegen);
            bench::<OxcParser>(&mut g, &source, without_codegen);
            if without_codegen {
                bench::<SwcFastTsStrip>(&mut g, &source, without_codegen);
            }
        }
        g.finish();
    }
}

criterion_group!(transpile, transpile_benchmark);
criterion_main!(transpile);
