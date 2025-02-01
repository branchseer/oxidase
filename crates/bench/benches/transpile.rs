use std::{cell::RefCell, path::Path};

use criterion::{measurement::WallTime, *};
use oxidase_bench::{Benchee, OxcParser, Oxidase, SwcFastTsStrip};

fn remove_codegen(source: &str) -> String {
    use oxidase_tsc::Tsc;
    thread_local! { static TSC: RefCell<Tsc> = RefCell::new(Tsc::new()) }
    TSC.with_borrow_mut(|tsc| tsc.process_ts(source, true, true))
        .unwrap()
        .ts
}

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
    let id = BenchmarkId::new(
        B::NAME,
        format!("{} ({:.3} MB)", name, size_mb)
    );
    let mut source_buf = String::new();
    g.bench_function(id, |b| {
        source_buf.clear();
        source_buf.push_str(source);
        b.iter_with_setup_wrapper(|runner| runner.run(|| benchee.run(&mut source_buf)));
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
