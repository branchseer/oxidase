use std::{
    cell::RefCell, fmt::Display, fs::read_to_string, path::{Path, PathBuf}, sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    }, time::Duration
};

use ignore::WalkBuilder;
use oxidase::{Allocator, SourceType, TranspileOptions};
use oxidase_tsc::{SourceKind, Tsc};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use similar_asserts::SimpleDiff;

pub struct Failure {
    pub path: PathBuf,
    pub input: String,
    pub kind: FailureKind,
}

pub enum FailureKind {
    OutputInvalidSyntax(String),
    UnmatchedOutput { expected: String, actual: String },
}

impl Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("## {}\n", self.path.display()))?;
        match &self.kind {
            FailureKind::OutputInvalidSyntax(output) => {
                f.write_str("output_invalid_syntax\n\n")?;
                f.write_str(&output)?;
            },
            FailureKind::UnmatchedOutput { expected, actual } => {
                SimpleDiff::from_str(expected, actual, "expected", "actual").fmt(f)?;
            }
        }
        f.write_str("\n")?;
        Ok(())
    }
}

fn main() {
    let test_repos_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("test_repos/typescript/tests/cases/compiler");
    let walk = WalkBuilder::new(test_repos_path)
        .standard_filters(false)
        .build_parallel();

    let all_paths: Mutex<Vec<PathBuf>> = Default::default();

    walk.run(|| {
        struct CollectOnDrop<'a> {
            paths: Vec<PathBuf>,
            all_paths: &'a Mutex<Vec<PathBuf>>,
        }
        impl<'a> Drop for CollectOnDrop<'a> {
            fn drop(&mut self) {
                self.all_paths.lock().unwrap().extend(self.paths.drain(..));
            }
        }
        let mut collect_on_drop = CollectOnDrop {
            paths: vec![],
            all_paths: &all_paths,
        };

        Box::new(move |entry| {
            let entry = entry.unwrap();
            let file_name = entry.file_name().as_encoded_bytes();
            if !(file_name.ends_with(b".js")
                || file_name.ends_with(b".mjs")
                || file_name.ends_with(b".ts")
                || file_name.ends_with(b".mts"))
            {
                return ignore::WalkState::Continue;
            }
            collect_on_drop.paths.push(entry.into_path());
            ignore::WalkState::Continue
        })
    });
    let mut all_paths = all_paths.into_inner().unwrap();

    // all_paths.retain(|path| path.file_name().unwrap() == "typeParameterLeak.ts");
    all_paths.sort_unstable();
    all_paths.truncate(200);

    let all_cnt = all_paths.len();
    let finished_cnt = Arc::new(AtomicUsize::new(0));
    std::thread::spawn({
        let finished_cnt = finished_cnt.clone();
        move || loop {
            let finished_cnt = finished_cnt.load(Ordering::Relaxed);
            if finished_cnt == all_cnt {
                println!("Done");
                break;
            }
            let precentage = (finished_cnt as f64 / all_cnt as f64) * 100.0;
            println!("{:.2}% ({}/{})...", precentage, finished_cnt, all_cnt);
            std::thread::sleep(Duration::from_secs_f64(0.5));
        }
    });

    thread_local! { static TSC: RefCell<Option<Tsc>> = RefCell::new(None) }
    thread_local! { static ALLOCATOR: RefCell<Option<Allocator>> = RefCell::new(None) }
    struct IncreOnDrop<'a>(&'a AtomicUsize);
    impl<'a> Drop for IncreOnDrop<'a> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let failures: Vec<Failure> = all_paths
        .par_iter()
        .flat_map(|path| -> Option<Failure> {
            let _incre_on_drop = IncreOnDrop(&finished_cnt);
            let Ok(source) = read_to_string(path) else {
                return None;
            };

            TSC.with_borrow_mut(|tsc| {
                let tsc = tsc.get_or_insert_with(|| Tsc::new());
                let Some(process_result) = tsc.process_ts(&source) else {
                    return None;
                };


                let source_type = match process_result.kind {
                    SourceKind::Module => SourceType::ts().with_module(true),
                    SourceKind::Script => SourceType::ts().with_module(false),
                };

                ALLOCATOR.with_borrow_mut(|allocator| {
                    let allocator = allocator.get_or_insert_with(|| Allocator::default());
                    let mut source = oxidase::Source::Borrowed(&process_result.ts);

                    let transpile_return = oxidase::transpile(
                        allocator,
                        TranspileOptions {
                            source_type,
                            prefer_blank_space: true,
                        },
                        &mut source,
                    );
                    if transpile_return.panicked {
                        // Ignore oxc parser error. it should be covered by oxc_parser's conformance tests
                        return None;
                    }
                    let output = source.as_str();
                    let Some(formated_output) = tsc.format_js(output) else {
                        return Some(Failure {
                            path: path.clone(),
                            input: process_result.ts.clone(),
                            kind: FailureKind::OutputInvalidSyntax(output.to_string()),
                        });
                    };
                    if formated_output != process_result.js {
                        return Some(Failure {
                            path: path.clone(),
                            input: process_result.ts,
                            kind: FailureKind::UnmatchedOutput {
                                expected: process_result.js,
                                actual: formated_output,
                            },
                        });
                    }
                    // let formated_output =
                    None
                })
            })
        }).map(|failure| {
            println!("{}", &failure);
            failure
        })
        .collect();

    dbg!(failures.len());
}
