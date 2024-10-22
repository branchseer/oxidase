#[path = "../common.rs"]
mod common;
mod ts;

use crate::common::{generated_folder_path, test_repos_path, SourceRecord};
use oxidase::ModuleKind;
use rayon::prelude::*;
use rayon::{in_place_scope, Scope};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use swc_common::{FileName, SourceMap};
use swc_ecma_ast::EsVersion;
use swc_ecma_parser::{parse_file_as_program, EsSyntax, Syntax};
use thread_local::ThreadLocal;
use ts::transpile_ts;

fn walk<'a>(
    scope: &Scope<'a>,
    folder_path: &Path,
    filter: &'a (impl Fn(&Path) -> bool + Send + Sync),
    on_file: &'a (impl Fn(&Path) + Send + Sync),
) {
    for entry in fs::read_dir(folder_path).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let full_path = entry.path();
        if file_type.is_file() {
            if filter(&full_path) {
                scope.spawn(move |scope| {
                    on_file(&full_path);
                });
            }
        } else if file_type.is_dir() {
            scope.spawn(move |scope| walk(scope, &full_path, filter, on_file))
        }
    }
}

fn main() {
    let scan_root = test_repos_path();
    let last_id = AtomicU64::new(0);
    let tls_records: ThreadLocal<RefCell<Vec<SourceRecord>>> = ThreadLocal::new();
    let generated_folder = generated_folder_path();
    let _ = fs::remove_dir_all(&generated_folder);
    fs::create_dir(&generated_folder).unwrap();

    let incompatible_js_paths =
        fs::read_to_string(scan_root.join("incompatible_js_paths.txt")).unwrap();
    let incompatible_js_paths = incompatible_js_paths
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                Some(line)
            }
        })
        .collect::<HashSet<&str>>();

    let handle_file = |full_path: &Path| {
        let id = last_id.fetch_add(1, Ordering::Relaxed);
        let name = full_path.file_name().unwrap().as_encoded_bytes();
        let path = full_path
            .strip_prefix(&scan_root)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let is_incompatible_js = incompatible_js_paths.contains(path.as_str());
        let module_kind = {
            let Ok(mut source) = fs::read_to_string(full_path) else {
                return;
            };
            let Some(transpile_ret) = transpile_ts(full_path, &mut source) else {
                return;
            };
            fs::write(&generated_folder.join(format!("{}.input.txt", id)), source).unwrap();
            fs::write(
                &generated_folder.join(format!("{}.output.txt", id)),
                transpile_ret.code,
            )
            .unwrap();
            transpile_ret.module_kind
        };
        tls_records
            .get_or(|| Default::default())
            .borrow_mut()
            .push(SourceRecord {
                id,
                path,
                module_kind,
            })
    };
    in_place_scope(|scope| {
        walk(
            scope,
            &scan_root,
            &|path| {
                let name = path.file_name().unwrap().as_encoded_bytes();
                (name.ends_with(b".ts") && !name.ends_with(b".d.ts"))
                    || (name.ends_with(b".mts") && !name.ends_with(b".d.mts"))
                    || (name.ends_with(b".cts") && !name.ends_with(b".d.cts"))
            },
            &handle_file,
        );
    });
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_path(generated_folder.join("_list.csv"))
        .unwrap();
    for records in tls_records.into_iter() {
        for record in records.into_inner() {
            writer.serialize(record).unwrap()
        }
    }
    writer.flush().unwrap()
}
