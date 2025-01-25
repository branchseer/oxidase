use std::path::Path;

use criterion::{measurement::WallTime, *};
use oxidase_bench::{remove_codegen, Benchee, OxcParser, Oxidase, SwcFastTsStrip};

fn bench<B: Benchee>(g: &mut BenchmarkGroup<'_, WallTime>, source: &str) {
    let mut benchee = B::default();
    g.bench_function(B::NAME, |b| {
        b.iter_batched(
            || source.to_string(),
            |mut source| {
                benchee.run(&mut source)
            },
            BatchSize::SmallInput
        )
    });
}

fn transformer_benchmark(c: &mut Criterion) {
    let filenames = ["checker.ts", "render.ts"];
    for filename in filenames {
        let path = Path::new("files").join(filename);
        let source = std::fs::read_to_string(&path).unwrap();

        for without_codegen in [false, true] {
            let mut group_name = filename.to_string();
            if without_codegen {
                group_name.push_str(" without codegen");
            }

            let mut g = c.benchmark_group(&group_name);
            let source = if without_codegen {
                remove_codegen(&source)
            } else {
                source.clone()
            };

            bench::<Oxidase>(&mut g, &source);
            bench::<OxcParser>(&mut g, &source);
            if without_codegen {
                bench::<SwcFastTsStrip>(&mut g, &source);
            }
            g.finish();
        }
    }
}

criterion_group!(transformer, transformer_benchmark);
criterion_main!(transformer);
