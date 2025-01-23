mod cache;
mod exec;
mod format;

use std::{
    any::Any,
    cell::RefCell,
    fmt::{Debug, Display, Write},
    fs::read_to_string,
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime},
};

use cache::BaselineCache;
use derive_more::From;
use exec::{eval, EvalError};
use format::format_js;
use ignore::{DirEntry, WalkBuilder};
use oxidase::{
    line_term::line_terminator_start_iter, Allocator, oxc_diagnostics::OxcDiagnostic, SourceType,
};
use oxidase_tsc::{SourceKind, Tsc};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use similar_asserts::SimpleDiff;
use thread_local::ThreadLocal;

pub struct Failure {
    pub path: String,
    pub input: String,
    pub kind: FailureKind,
}

#[derive(From)]
pub enum FailureKind {
    IoError(io::Error),
    OutputInvalidSyntax(String),
    TscInvalidSyntax,
    FormatTscOutputError(anyhow::Error),
    ParserPanicked(Vec<OxcDiagnostic>),
    Panicked(Box<dyn Any + Send>),
    InputInvalidSyntax(String),
    UnmatchedOutput {
        expected: String,
        actual: String,
    },
    FormatTscOutputPanicked(Box<dyn Any + Send>),
    LineTerminatorCountMisMatch {
        source_line_term_starts: Vec<usize>,
        output_line_term_starts: Vec<usize>,
    },
    #[from]
    ExecEvalError(EvalError),

    ExecOutputNotEqual {
        expected: serde_json::Value,
        actual: serde_json::Value,
    },
}

fn get_panic_message(panic_err: &dyn Any) -> String {
    if let Some(message) = panic_err.downcast_ref::<&'static str>() {
        String::from(*message)
    } else if let Some(message) = panic_err.downcast_ref::<String>() {
        message.clone()
    } else {
        String::from("unknown message")
    }
}

impl Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("## {}\n", self.path))?;
        match &self.kind {
            FailureKind::OutputInvalidSyntax(output) => {
                f.write_str("output_invalid_syntax\n\n")?;
                f.write_str(&output)?;
            }
            FailureKind::InputInvalidSyntax(input) => {
                f.write_str("input_invalid_syntax\n\n")?;
                f.write_str(&input)?;
            }
            FailureKind::TscInvalidSyntax => {
                f.write_str("tsc_invalid_syntax\n\n")?;
            }
            FailureKind::UnmatchedOutput { expected, actual } => {
                SimpleDiff::from_str(expected, actual, "expected", "actual").fmt(f)?;
            }
            FailureKind::Panicked(panic_err) => {
                let panic_message = get_panic_message(panic_err.as_ref());
                f.write_fmt(format_args!("Transpiler panicked: {}", panic_message))?;
            }
            FailureKind::LineTerminatorCountMisMatch {
                source_line_term_starts,
                output_line_term_starts,
            } => {
                f.write_fmt(format_args!(
                    "Line terminator count mismatch (source: {}, output: {})",
                    source_line_term_starts.len(),
                    output_line_term_starts.len()
                ))?;
                f.write_fmt(format_args!(
                    "Source line terminator starts: {:?}",
                    source_line_term_starts,
                ))?;
                f.write_fmt(format_args!(
                    "Output line terminator starts: {:?}",
                    output_line_term_starts,
                ))?;
            }
            FailureKind::ExecEvalError(eval_error) => Display::fmt(eval_error, f)?,
            FailureKind::ExecOutputNotEqual { expected, actual } => {
                f.write_str("ExecOutputNotEqual\n")?;
                pretty_assertions::Comparison::new(expected, actual).fmt(f)?;
            }
            FailureKind::IoError(error) => {
                f.write_str("io error\n")?;
                Debug::fmt(error, f)?;
            }
            FailureKind::FormatTscOutputError(error) => {
                f.write_str("FormatTscOutputError\n")?;
                Debug::fmt(error, f)?;
            }
            FailureKind::FormatTscOutputPanicked(panic_err) => {
                f.write_str("FormatTscOutputError\n")?;
                f.write_str(&get_panic_message(panic_err.as_ref()))?;
            }
            FailureKind::ParserPanicked(diagnostics) => {
                f.write_str("ParserPanicked\n")?;
                Debug::fmt(&diagnostics, f)?;
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
    filter_map: impl (Fn(DirEntry) -> Option<E>) + Send + Sync,
    entries: &mut Vec<E>,
) {
    let tls_entries: ThreadLocal<RefCell<Vec<E>>> = Default::default();
    let walk = WalkBuilder::new(path)
        .standard_filters(false)
        .build_parallel();
    walk.run(|| {
        Box::new(|dir_entry| {
            let dir_entry = dir_entry.unwrap();
            if let Some(entry) = filter_map(dir_entry) {
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

#[derive(Debug, PartialEq, Eq)]
enum TestType {
    Transpile,
    Exec,
}

fn main() {
    let transpile_fixture_dirs =
        read_to_string(Path::new(FIXTURE_PATH).join("ecosystem_transpile_paths.txt")).unwrap();
    let mut paths_allowing_invalid_ts: Vec<&str> = vec![];
    let transpile_fixture_dirs = transpile_fixture_dirs.lines().flat_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            None
        } else {
            Some(if let Some(line) = line.strip_prefix('?') {
                paths_allowing_invalid_ts.push(line);
                line
            } else {
                line
            })
        }
    });

    let mut entries = Vec::<FileEntry>::new();

    fn filter_map(dir_entry: DirEntry) -> Option<FileEntry> {
        let path = dir_entry.path().to_str()?;
        if path.ends_with(".js") || path.ends_with(".mjs") || path.ends_with(".ts") {
            Some(FileEntry {
                full_path: path.to_owned(),
                mtime: dir_entry.metadata().unwrap().modified().unwrap(),
            })
        } else {
            None
        }
    }
    for transpile_fixture_dir in transpile_fixture_dirs {
        collect_entries(
            Path::new(FIXTURE_PATH).join(transpile_fixture_dir),
            filter_map,
            &mut entries,
        );
    }
    collect_entries(
        Path::new(FIXTURE_PATH).join("exec"),
        filter_map,
        &mut entries,
    );
    collect_entries(
        Path::new(FIXTURE_PATH).join("transpile"),
        filter_map,
        &mut entries,
    );

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
            let res = TSC.with_borrow_mut(|tsc| -> Result<(), Failure> {
                let path = file_entry.full_path.strip_prefix(FIXTURE_PATH).unwrap();
                let allows_invalid_js = || {
                    paths_allowing_invalid_ts
                        .iter()
                        .any(|prefix| Path::new(path).starts_with(prefix))
                };

                let source = match read_to_string(&file_entry.full_path) {
                    Ok(ok) => ok,
                    Err(io_error) => {
                        return if allows_invalid_js() {
                            Ok(())
                        } else {
                            Err(Failure {
                                path: path.to_owned(),
                                input: String::new(),
                                kind: FailureKind::IoError(io_error),
                            })
                        }
                    }
                };

                let test_type = if Path::new(path).starts_with("exec") {
                    TestType::Exec
                } else {
                    TestType::Transpile
                };

                let tsc = tsc.get_or_insert_with(|| Tsc::new());

                let Some(tsc_output) = (match test_type {
                    TestType::Exec => tsc.process_ts(&source, false, false),
                    TestType::Transpile => {
                        baseline_cache.get_or_insert_with(path, file_entry.mtime, || {
                            tsc.process_ts(&source, true, false)
                        })
                    }
                }) else {
                    return if allows_invalid_js() {
                        Ok(())
                    } else {
                        Err(Failure {
                            path: path.to_owned(),
                            input: source.clone(),
                            kind: FailureKind::TscInvalidSyntax,
                        })
                    };
                };

                let source_type = match tsc_output.kind {
                    SourceKind::Module => SourceType::ts().with_module(true),
                    SourceKind::Script => SourceType::ts().with_module(false),
                };

                let input = match test_type {
                    TestType::Transpile if Path::new(path).starts_with("ecosystem") => {
                        tsc_output.ts
                    }
                    _ => source,
                };

                ALLOCATOR
                    .with_borrow_mut(|allocator| -> Result<(), FailureKind> {
                        let allocator = allocator.get_or_insert_with(|| Allocator::default());
                        allocator.reset();

                        let mut output = input.clone();
                        let transpile_return = match catch_unwind(AssertUnwindSafe(|| {
                            oxidase::transpile(allocator, source_type, &mut output)
                        })) {
                            Ok(ok) => ok,
                            Err(panic_err) => {
                                return Err(FailureKind::Panicked(panic_err));
                            }
                        };
                        match test_type {
                            TestType::Transpile => {
                                let expected_output = match catch_unwind(|| {
                                    format_js(&tsc_output.js)
                                }) {
                                    Ok(Ok(ok)) => ok,
                                    Err(panic_error) => {
                                        return if allows_invalid_js() {
                                            Ok(())
                                        } else {
                                            Err(FailureKind::FormatTscOutputPanicked(panic_error))
                                        }
                                    }
                                    Ok(Err(swc_error)) => {
                                        return if allows_invalid_js() {
                                            Ok(())
                                        } else {
                                            Err(FailureKind::FormatTscOutputError(swc_error))
                                        }
                                    }
                                };
                                if transpile_return.parser_panicked {
                                    return if allows_invalid_js() {
                                        Ok(())
                                    } else {
                                        Err(FailureKind::ParserPanicked(
                                            transpile_return.parser_errors,
                                        ))
                                    };
                                }

                                let output = output.as_str();

                                let Ok(formated_output) = format_js(output) else {
                                    if allows_invalid_js() {
                                        return Ok(());
                                    }
                                    return Err(FailureKind::OutputInvalidSyntax(
                                        output.to_string(),
                                    ));
                                };

                                if formated_output != expected_output {
                                    return Err(FailureKind::UnmatchedOutput {
                                        expected: expected_output,
                                        actual: formated_output,
                                    });
                                }
                            }
                            TestType::Exec => {
                                assert!(!transpile_return.parser_panicked);
                                let expected_exports = eval(&tsc_output.js)?;
                                let actual_exports = eval(&output)?;
                                if expected_exports != actual_exports {
                                    return Err(FailureKind::ExecOutputNotEqual {
                                        expected: expected_exports,
                                        actual: actual_exports,
                                    });
                                }
                            }
                        }
                        let source_line_term_starts =
                            line_terminator_start_iter(input.as_bytes()).collect::<Vec<usize>>();
                        let output_line_term_starts =
                            line_terminator_start_iter(output.as_bytes()).collect::<Vec<usize>>();
                        if output_line_term_starts.len() != source_line_term_starts.len() {
                            return Err(FailureKind::LineTerminatorCountMisMatch {
                                source_line_term_starts,
                                output_line_term_starts,
                            });
                        }
                        // let formated_output =
                        Ok(())
                    })
                    .map_err(|failure_kind| Failure {
                        path: path.to_owned(),
                        input,
                        kind: failure_kind,
                    })
            });
            finished_cnt.fetch_add(1, Ordering::Relaxed);
            res.err()
        })
        .map(|failure| {
            println!("{}", &failure);
            failure
        })
        .collect();

    dbg!(failures.len());
    baseline_cache.save();
}
