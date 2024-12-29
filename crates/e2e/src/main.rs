mod cache;
mod format_ts;

use std::{
    any::Any, cell::RefCell, fmt::{Debug, Display}, fs::read_to_string, os::unix::fs::MetadataExt, panic::{catch_unwind, AssertUnwindSafe}, path::{Path, PathBuf}, sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    }, time::{Duration, SystemTime}
};

use cache::BaselineCache;
use format_ts::format_js;
use ignore::{DirEntry, WalkBuilder};
use oxidase::{Allocator, SourceType, String as AllocatorString};
use oxidase_tsc::{SourceKind, Tsc};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use similar_asserts::SimpleDiff;
use thread_local::ThreadLocal;


pub struct Failure {
    pub path: String,
    pub input: String,
    pub kind: FailureKind,
}

pub enum FailureKind {
    OutputInvalidSyntax(String),
    UnmatchedOutput { expected: String, actual: String },
    Panicked(Box<dyn Any + Send>)
}

impl Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("## {}\n", self.path))?;
        match &self.kind {
            FailureKind::OutputInvalidSyntax(output) => {
                f.write_str("output_invalid_syntax\n\n")?;
                f.write_str(&output)?;
            }
            FailureKind::UnmatchedOutput { expected, actual } => {
                SimpleDiff::from_str(expected, actual, "expected", "actual").fmt(f)?;
            },
            FailureKind::Panicked(panic_err) => {
                let panic_message = if let Some(message) = panic_err.downcast_ref::<&'static str>() {
                    *message
                } else if let Some(message) =  panic_err.downcast_ref::<String>() {
                    message.as_str()
                } else {
                    "unknown message"
                };
                f.write_fmt(format_args!("Transpiler panicked: {}", panic_message))?;
            }
        }
        f.write_str("\n")?;
        Ok(())
    }
}

pub const FIXTURE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixture/");
pub const BASELINE_CACHE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/baseline_cache/");

fn collect_entries<E: Send>(
    path: impl AsRef<Path>,
    flat_map: impl (Fn(DirEntry) -> Option<E>) + Send + Sync,
    entries: &mut Vec<E>,
) {
    let tls_entries: ThreadLocal<RefCell<Vec<E>>> = Default::default();
    let walk = WalkBuilder::new(path)
        .standard_filters(false)
        .build_parallel();
    walk.run(|| {
        Box::new(|dir_entry| {
            let dir_entry = dir_entry.unwrap();
            if let Some(entry) = flat_map(dir_entry) {
                let entries = tls_entries.get_or_default();
                let mut entries = entries.borrow_mut();
                entries.push(entry);
            }
            ignore::WalkState::Continue
        })
    });

    for tls_entries in tls_entries {
        entries.extend(tls_entries.into_inner());
    }
}

pub struct FileEntry {
    pub full_path: String,
    pub mtime: SystemTime,
}

fn main() {
    let transpile_fixture_dirs =
        read_to_string(Path::new(FIXTURE_PATH).join("transpile_list.txt")).unwrap();
    let transpile_fixture_dirs = transpile_fixture_dirs.lines().flat_map(|line| {
        let line = line.trim();
        if line.is_empty() {
            None
        } else {
            Some(line)
        }
    });

    let mut entries = Vec::<FileEntry>::new();
    for transpile_fixture_dir in transpile_fixture_dirs {
        collect_entries(
            Path::new(FIXTURE_PATH).join(transpile_fixture_dir),
            |dir_entry| {
                let path = dir_entry.path().to_str()?;
                if path.ends_with(".js") || path.ends_with(".mjs") || path.ends_with(".ts") {
                    Some(FileEntry {
                        full_path: path.to_owned(),
                        mtime: dir_entry.metadata().unwrap().modified().unwrap(),
                    })
                } else {
                    None
                }
            },
            &mut entries,
        );
    }

    // all_paths.retain(|path| path.file_name().unwrap() == "typeParameterLeak.ts");
    entries.sort_by(|entry1, entry2| entry1.full_path.cmp(&entry2.full_path));

    let entries = &entries;

    let all_cnt = entries.len();
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

    let baseline_cache = BaselineCache::new(BASELINE_CACHE_PATH);

    let failures: Vec<Failure> = entries
        .par_iter()
        .flat_map(|file_entry| -> Option<Failure> {
            let ret = TSC.with_borrow_mut(|tsc| {
                let Ok(source) = read_to_string(&file_entry.full_path) else {
                    return None;
                };

                let path = file_entry.full_path.strip_prefix(FIXTURE_PATH).unwrap();

                let tsc = tsc.get_or_insert_with(|| Tsc::new());
                let Some(process_result) =
                    baseline_cache.get_or_insert_with(path, file_entry.mtime, || {
                        tsc.process_ts(&source, true)
                    })
                else {
                    return None;
                };

                let source_type = match process_result.kind {
                    SourceKind::Module => SourceType::ts().with_module(true),
                    SourceKind::Script => SourceType::ts().with_module(false),
                };

                ALLOCATOR.with_borrow_mut(|allocator| {
                    let allocator = allocator.get_or_insert_with(|| Allocator::default());
                    allocator.reset();
                    let mut source = process_result.ts.clone();

                    let transpile_return = match catch_unwind(AssertUnwindSafe(|| oxidase::transpile(
                        allocator,
                            source_type,
                        &mut source,
                    ))) {
                        Ok(ok) => ok,
                        Err(panic_err) => {
                            return Some(Failure {
                                path: path.to_owned(),
                                input: process_result.ts,
                                kind: FailureKind::Panicked(panic_err),
                            });
                        }
                    };
                    if transpile_return.parser_panicked {
                        // Ignore oxc parser error. it should be covered by oxc_parser's conformance tests
                        return None;
                    }

                    let Ok(Ok(expected_output)) = catch_unwind(|| format_js(&process_result.js)) else {
                        // eprintln!("swc err formating expected output {}", path);
                        // Ignore invalid expected js output
                        return None;
                    };
                    let output = source.as_str();

                    let Ok(formated_output) = format_js(output) else {
                        if !transpile_return.parser_errors.is_empty() {
                            return None;
                        }
                        return Some(Failure {
                            path: path.to_owned(),
                            input: process_result.ts.clone(),
                            kind: FailureKind::OutputInvalidSyntax(output.to_string()),
                        });
                    };

                    if formated_output != expected_output {
                        return Some(Failure {
                            path: path.to_owned(),
                            input: process_result.ts,
                            kind: FailureKind::UnmatchedOutput {
                                expected: expected_output,
                                actual: formated_output,
                            },
                        });
                    }
                    // let formated_output =
                    None
                })
            });
            finished_cnt.fetch_add(1, Ordering::Relaxed);
            ret
        })
        .map(|failure| {
            println!("{}", &failure);
            failure
        })
        .collect();

    dbg!(failures.len());
    baseline_cache.save();
}
