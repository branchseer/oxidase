// mod generate;

mod common;
mod format_ts;

use crate::common::{generated_folder_path, test_repos_path, SourceRecord};
use crate::format_ts::format_ts;
use core::str;
use googletest::{prelude::*, test};
use oxidase::{transpile, Allocator, Source, SourceType, TranspileOptions};
// use oxidase::{diagnostic::Strict, transpile};
use rayon::prelude::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use swc_common::Spanned;

struct OngoingRemover(u64, Arc<Mutex<HashSet<u64>>>);
impl Drop for OngoingRemover {
    fn drop(&mut self) {
        self.1.lock().unwrap().remove(&self.0);
    }
}

#[test]
fn oxidase_e2e() {
    let generated_folder = generated_folder_path();
    let test_repos_folder = test_repos_path();
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(generated_folder.join("_list.csv"))
        .unwrap();

    let files = csv_reader
        .deserialize::<SourceRecord>()
        .collect::<csv::Result<Vec<SourceRecord>>>()
        .unwrap();

    fn to_failure(result: Result<()>) -> Option<Result<Infallible>> {
        match result {
            Ok(()) => None,
            Err(err) => Some(Err(err)),
        }
    }
    let ongoing_ids: Arc<Mutex<HashSet<u64>>> = Default::default();
    std::thread::spawn({
        let ongoing_ids = std::sync::Arc::clone(&ongoing_ids);
        move || {
            let mut snapshot_before = ongoing_ids.lock().unwrap().clone();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let snapshot = ongoing_ids.lock().unwrap().clone();
                for id in snapshot.intersection(&snapshot_before) {
                    println!("{} took more than 5 second", id);
                }
                snapshot_before = snapshot;
            }
        }
    });
    let count = AtomicUsize::new(0);
    let linked_failures = files
        .into_par_iter()
        .filter_map(|record| {
            count.fetch_add(1, Ordering::Relaxed);
            ongoing_ids.lock().unwrap().insert(record.id);
            let _logger = OngoingRemover(record.id, ongoing_ids.clone());
            thread_local! {
                static ALLOCATOR: Allocator = Allocator::default();
            }
            ALLOCATOR.with(|allocator| {
                let input =
                    fs::read_to_string(&generated_folder.join(format!("{}.input.txt", record.id)))
                        .unwrap();
                let expected_output =
                    fs::read_to_string(&generated_folder.join(format!("{}.output.txt", record.id)))
                        .unwrap();

                let mut source = Source::Borrowed(input.as_str());
                let source_type =
                    SourceType::ts().with_module(record.module_kind == oxidase::ModuleKind::Module);

                let transpile_ret = transpile(
                    allocator,
                    TranspileOptions {
                        source_type,
                        prefer_blank_space: true,
                    },
                    &mut source,
                );

                if !transpile_ret.errors.is_empty() {
                    return Some(
                        to_failure(fail!(
                            "Transpiling {} failed with errors: {:?}",
                            &record.path,
                            transpile_ret.errors
                        ))
                        .unwrap(),
                    );
                }
                // match transpile(record.module_kind, &input, out_buf, &diagnostic) {
                //     Ok(()) => {}
                //     Err(error) => {
                //         return Some(
                //             to_failure(fail!(
                //                 "Transpiling {} failed with error: {:?}",
                //                 &record.path,
                //                 error
                //             ))
                //             .unwrap(),
                //         )
                //     }
                // };

                let formatted_out = match format_ts(source.as_str(), record.module_kind) {
                    Ok(ok) => ok,
                    Err(err) => {
                        return Some(
                            to_failure(fail!(
                                "Format the output of {} failed.\n----- Output -----\n{}\n----- error -----n{:?}",
                                &record.path,
                                source.as_str(),
                                err
                            ))
                            .unwrap(),
                        )
                    }
                };
                to_failure(
                    verify_eq!(&formatted_out, &expected_output).with_failure_message(move || {
                        format!(
                            "Incorrect transpile output for ts file({}): {}",
                            record.id, record.path
                        )
                    }),
                )
            })
        })
        .collect_vec_list();
    let mut failure_count = 0usize;
    for failures in linked_failures {
        failure_count += failures.len();
        failures
            .into_iter()
            .for_each(GoogleTestSupport::and_log_failure);
    }
    if failure_count > 0 {
        let total_count = count.load(Ordering::Relaxed);
        let percentage = failure_count as f64 / total_count as f64 * 100.0;
        fail!(
            "{}% ({}/{}) tests failed",
            percentage,
            failure_count,
            total_count
        )
        .and_log_failure();
    }
}
