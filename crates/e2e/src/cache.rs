use std::{
    cmp::max,
    fs::{self, read_to_string},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

use dashmap::DashMap;
use oxidase_tsc::{SourceKind, TranspileOutput};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct CacheEntry {
    id: u64,
    mtime: u64,
    /// None means the transiple failed
    source_kind: Option<SourceKind>,
}

#[derive(Deserialize, Serialize)]
struct CacheCsvLine {
    path: String,
    entry: CacheEntry,
}

pub struct BaselineCache {
    cache_root: PathBuf,
    entries: DashMap<String, CacheEntry>,
    max_id: AtomicU64,
}

impl BaselineCache {
    pub fn new(cache_root: impl Into<PathBuf>) -> Self {
        let cache_root = cache_root.into();
        let _ = fs::create_dir(&cache_root);
        let mut max_id = 0u64;
        let entries = DashMap::<String, CacheEntry>::default();
        if let Ok(reader) = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(list_path(&cache_root))
        {
            for line in reader.into_deserialize::<CacheCsvLine>() {
                let line = line.unwrap();
                max_id = max(max_id, line.entry.id);
                entries.insert(line.path, line.entry);
            }
        }
        Self {
            entries,
            cache_root,
            max_id: AtomicU64::new(max_id),
        }
    }
    pub fn get_or_insert_with(
        &self,
        path: &str,
        mtime: SystemTime,
        f: impl FnOnce() -> Option<TranspileOutput>,
    ) -> Option<TranspileOutput> {
        let mtime = time_to_num(mtime);
        if let Some(entry) = self.entries.get(path) {
            let entry = entry.value();
            if entry.mtime == mtime {
                let kind = entry.source_kind?;
                let js = read_to_string(self.js_path(entry.id)).unwrap();
                let ts = read_to_string(self.ts_path(entry.id)).unwrap();
                return Some(TranspileOutput { js, ts, kind });
            }
        }
        let output = f();
        self.entries.insert(
            path.to_owned(),
            if let Some(output) = &output {
                let id = self.max_id.fetch_add(1, Ordering::Relaxed) + 1;
                fs::write(self.js_path(id), &output.js).unwrap();
                fs::write(self.ts_path(id), &output.ts).unwrap();
                CacheEntry {
                    id,
                    mtime,
                    source_kind: Some(output.kind),
                }
            } else {
                CacheEntry {
                    id: 0,
                    mtime,
                    source_kind: None,
                }
            },
        );
        output
    }
    pub fn save(self) {
        let mut csv_writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_path(list_path(&self.cache_root))
            .unwrap();
        for (path, entry) in self.entries {
            csv_writer.serialize(CacheCsvLine { path, entry }).unwrap();
        }
    }
    fn js_path(&self, id: u64) -> PathBuf {
        self.cache_root.join(format!("{}.js.txt", id))
    }
    fn ts_path(&self, id: u64) -> PathBuf {
        self.cache_root.join(format!("{}.ts.txt", id))
    }
}

fn list_path(cache_root: &Path) -> PathBuf {
    cache_root.join("_list.csv")
}

fn time_to_num(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .try_into()
        .unwrap()
}
