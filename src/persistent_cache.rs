use crate::engine::SearchReport;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PersistentBound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentCacheEntry {
    pub board_key: String,
    pub depth: u32,
    pub score_white: i32,
    pub best_move: Option<String>,
    pub bound: PersistentBound,
    pub timestamp_unix_secs: u64,
}

#[derive(Default)]
pub struct PersistentAnalysisCache {
    path: PathBuf,
    entries: HashMap<String, PersistentCacheEntry>,
    pub hits: u64,
}

impl PersistentAnalysisCache {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut cache = Self {
            path,
            entries: HashMap::new(),
            hits: 0,
        };
        if let Ok(content) = fs::read_to_string(&cache.path) {
            if let Ok(values) = serde_json::from_str::<Vec<PersistentCacheEntry>>(&content) {
                cache.entries = values
                    .into_iter()
                    .map(|v| (v.board_key.clone(), v))
                    .collect();
            }
        }
        cache
    }

    pub fn lookup(&mut self, board_key: &str, required_depth: u32) -> Option<PersistentCacheEntry> {
        let entry = self.entries.get(board_key)?;
        if entry.depth >= required_depth {
            self.hits += 1;
            Some(entry.clone())
        } else {
            None
        }
    }

    pub fn upsert_root(&mut self, board_key: String, report: &SearchReport) {
        self.entries.insert(
            board_key.clone(),
            PersistentCacheEntry {
                board_key,
                depth: report.completed_depth,
                score_white: report.score_white,
                best_move: report.principal_variation.first().cloned(),
                bound: PersistentBound::Exact,
                timestamp_unix_secs: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            },
        );
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        let _ = fs::remove_file(&self.path);
    }

    pub fn flush_atomic(&self) {
        let mut values: Vec<_> = self.entries.values().cloned().collect();
        values.sort_by(|a, b| a.board_key.cmp(&b.board_key));
        let Ok(payload) = serde_json::to_vec_pretty(&values) else {
            return;
        };
        let tmp_path = self.path.with_extension("tmp");
        if fs::write(&tmp_path, payload).is_ok() {
            let _ = fs::rename(tmp_path, &self.path);
        }
    }
}
