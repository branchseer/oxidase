#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::{hint::black_box, path::Path, thread::spawn};

use criterion::{measurement::WallTime, *};
use oxidase_bench::{Benchee, OxcParser, Oxidase, SwcFastTsStrip};

use oxidase_tsc::Tsc;

fn bench<B: Benchee>(
    g: &mut BenchmarkGroup<'_, WallTime>,
    source: &str,
    is_source_without_codegen: bool,
) {
    let mut benchee = B::default();
    let name = if is_source_without_codegen {
        "erasable syntax only"
    } else {
        "original"
    };
    let size_mb = source.as_bytes().len() as f64 / 1024.0 / 1024.0;
    let id = BenchmarkId::new(B::NAME, format!("{} ({:.3} MB)", name, size_mb));
    let mut source_buf = String::new();
    g.bench_function(id, |b| {
        b.iter_with_setup_wrapper(|runner| {
            source_buf.clear();
            source_buf.push_str(source);
            runner.run(|| {
                benchee.run(&mut source_buf);
                black_box(source_buf.as_str());
            });
        });
    });
}

fn transpile_benchmark(c: &mut Criterion) {
    let filenames = ["checker.ts", "render.ts"];
    let sources: Vec<String> = filenames
        .iter()
        .map(|filename| std::fs::read_to_string(Path::new("files").join(filename)).unwrap())
        .collect();

    let erasable_syntax_only_sources: Vec<String> = spawn({
        let sources = sources.clone();
        move || {
            let mut tsc = Tsc::new();
            sources
                .iter()
                .map(|source| tsc.process_ts(source, true, true).unwrap().ts)
                .collect()
        }
    })
    .join()
    .unwrap();

    unsafe { v8::V8::dispose() };
    v8::V8::dispose_platform();

    for (index, filename) in filenames.iter().copied().enumerate() {
        let mut g = c.benchmark_group(filename);

        for erasable_syntax_only in [false, true] {
            let source = if erasable_syntax_only {
                erasable_syntax_only_sources[index].clone()
            } else {
                sources[index].clone()
            };

            bench::<Oxidase>(&mut g, &source, erasable_syntax_only);
            bench::<OxcParser>(&mut g, &source, erasable_syntax_only);
            if erasable_syntax_only {
                bench::<SwcFastTsStrip>(&mut g, &source, erasable_syntax_only);
            }
        }
        g.finish();
    }
}

criterion_group!(transpile, transpile_benchmark);
criterion_main!(transpile);
