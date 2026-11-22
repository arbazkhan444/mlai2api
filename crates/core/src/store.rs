//! Request history: metadata in JSONL, request/response bodies on disk with a cap.
use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const KEEP_IN_MEMORY: usize = 5000;
const ROTATE_ABOVE: usize = 20000;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
pub const TOKENS_PER_CREDIT: f64 = 1_000_000.0;
pub const UNATTRIBUTED_ID: &str = "unattributed";
pub const UNATTRIBUTED_LABEL: &str = "Unattributed";

fn default_account_id() -> String {
    UNATTRIBUTED_ID.to_string()
}
fn default_account_label() -> String {
    UNATTRIBUTED_LABEL.to_string()
}

#[derive(Debug, Clone)]
pub struct NewRequest {
    pub model: String,
    pub real_model: String,
    pub stream: bool,
    pub status: u16,
    pub ms: i64,
    pub account_id: String,
    pub account_label: String,
    pub effort: Option<String>,
    pub first_token_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRow {
    pub id: i64,
    pub ts: String,
    pub model: String,
    pub real_model: String,
    pub stream: bool,
    pub status: u16,
    pub ms: i64,
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_account_label")]
    pub account_label: String,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub first_token_ms: Option<i64>,
    #[serde(default)]
    pub has_body: bool,
    #[serde(default)]
    pub prompt_tokens: Option<i64>,
    #[serde(default)]
    pub completion_tokens: Option<i64>,
    #[serde(default)]
    pub cached_tokens: Option<i64>,
    #[serde(default)]
    pub reasoning_tokens: Option<i64>,
    #[serde(default)]
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub requests: i64,
    pub errors: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
    pub cache_hits: i64,
    pub credits: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub account_id: String,
    pub account_label: String,
    pub requests: i64,
    pub total_tokens: i64,
    pub credits: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub real_model: String,
    pub requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
    pub cache_hits: i64,
    pub credits: f64,
}

#[derive(Debug, Clone)]
pub struct BodyPair {
    pub request: Value,
    pub response: Value,
}

fn credits_of(tokens: i64) -> f64 {
    ((tokens as f64 / TOKENS_PER_CREDIT) * 1_000_000.0).round() / 1_000_000.0
}

impl Stats {
    fn add(&mut self, row: &RequestRow) {
        self.requests += 1;
        if row.status >= 400 {
            self.errors += 1;
        }
        let prompt = row.prompt_tokens.unwrap_or(0);
        let completion = row.completion_tokens.unwrap_or(0);
        let cached = row.cached_tokens.unwrap_or(0);
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.cached_tokens += cached;
        self.reasoning_tokens += row.reasoning_tokens.unwrap_or(0);
        self.total_tokens += row.total_tokens.unwrap_or(prompt + completion);
        if cached > 0 {
            self.cache_hits += 1;
        }
    }
    fn finish(&mut self) {
        self.credits = credits_of(self.total_tokens);
    }
}

struct Inner {
    rows: Vec<RequestRow>,
    next_id: i64,
    stats: Stats,
}

pub struct Store {
    path: PathBuf,
    bodies: PathBuf,
    body_keep: usize,
    body_max_bytes: u64,
    inner: Mutex<Inner>,
}

fn read_rows(path: &Path) -> Vec<RequestRow> {
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };
    let mut rows: Vec<RequestRow> = BufReader::new(file)
        .lines()
        .map_while(std::result::Result::ok)
        .filter_map(|line| serde_json::from_str::<RequestRow>(&line).ok())
        .collect();
    if rows.len() > ROTATE_ABOVE {
        rows = rows.split_off(rows.len() - KEEP_IN_MEMORY);
    }
    rows
}

fn body_id(path: &Path) -> Option<i64> {
    path.file_stem()?.to_str()?.parse::<i64>().ok()
}

impl Store {
    pub fn open(path: &Path, body_keep: i64, body_max_bytes: i64) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bodies = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("bodies");
        std::fs::create_dir_all(&bodies)?;
        let rows = read_rows(path);
        let mut stats = Stats::default();
        for row in rows.iter() {
            stats.add(row);
        }
        stats.finish();
        let next_id = rows.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        let store = Store {
            path: path.to_path_buf(),
            bodies,
            body_keep: body_keep.max(1) as usize,
            body_max_bytes: body_max_bytes.max(1) as u64,
            inner: Mutex::new(Inner { rows, next_id, stats }),
        };
        store.prune_bodies();
        Ok(store)
    }

    fn save_body(&self, id: i64, pair: &BodyPair) -> Result<()> {
        let mut text = serde_json::to_string(&json!({
            "request": pair.request,
            "response": pair.response,
        }))?;
        if text.len() > MAX_BODY_BYTES {
            text = serde_json::to_string(&json!({
                "truncated": true,
                "note": "body exceeded the 8 MB per-request cap",
                "request": pair.request,
                "responsePreview": pair.response.to_string().chars().take(200_000).collect::<String>(),
            }))?;
        }
        std::fs::write(self.bodies.join(format!("{}.json", id)), text)?;
        self.prune_bodies();
        Ok(())
    }

    /// Keep at most body_keep files and body_max_bytes of them, oldest first out.
    pub fn prune_bodies(&self) {
        let Ok(entries) = std::fs::read_dir(&self.bodies) else { return };
        let mut files: Vec<(i64, u64, PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    return None;
                }
                let id = body_id(&path)?;
                let size = entry.metadata().ok()?.len();
                Some((id, size, path))
            })
            .collect();
        files.sort_by_key(|(id, _, _)| *id);
        let mut total: u64 = files.iter().map(|(_, size, _)| *size).sum();
        let mut index = 0;
        while index < files.len()
            && (files.len() - index > self.body_keep || total > self.body_max_bytes)
        {
            let (_, size, path) = &files[index];
            let _ = std::fs::remove_file(path);
            total = total.saturating_sub(*size);
            index += 1;
        }
    }

    pub fn load_body(&self, id: i64) -> Result<Option<Value>> {
        let path = self.bodies.join(format!("{}.json", id));
        let Ok(text) = std::fs::read_to_string(&path) else { return Ok(None) };
        Ok(serde_json::from_str::<Value>(&text).ok())
    }

    fn append(&self, id: i64, row: &NewRequest, has_body: bool) -> Result<i64> {
        let record = {
            let mut inner = self.inner.lock().unwrap();
            let record = RequestRow {
                id,
                ts: chrono::Utc::now().to_rfc3339(),
                model: row.model.clone(),
                real_model: row.real_model.clone(),
                stream: row.stream,
                status: row.status,
                ms: row.ms,
                account_id: row.account_id.clone(),
                account_label: row.account_label.clone(),
                effort: row.effort.clone(),
                first_token_ms: row.first_token_ms,
                has_body,
                prompt_tokens: row.prompt_tokens,
                completion_tokens: row.completion_tokens,
                cached_tokens: row.cached_tokens,
                reasoning_tokens: row.reasoning_tokens,
                total_tokens: row.total_tokens,
                error: row.error.clone(),
            };
            inner.next_id = inner.next_id.max(id + 1);
            inner.stats.add(&record);
            inner.stats.finish();
            inner.rows.push(record.clone());
            if inner.rows.len() > KEEP_IN_MEMORY {
                let excess = inner.rows.len() - KEEP_IN_MEMORY;
                inner.rows.drain(..excess);
            }
            record
        };
        let line = serde_json::to_string(&record)? + "\n";
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        file.write_all(line.as_bytes())?;
        file.flush()?;
        Ok(id)
    }

    pub fn record(&self, row: &NewRequest, bodies: Option<BodyPair>) -> Result<i64> {
        let id = self.inner.lock().unwrap().next_id;
        let mut has_body = false;
        if let Some(pair) = bodies {
            match self.save_body(id, &pair) {
                Ok(()) => has_body = true,
                Err(error) => tracing::warn!("could not persist bodies for {}: {}", id, error),
            }
        }
        self.append(id, row, has_body)
    }

    pub fn get(&self, id: i64) -> Result<Option<RequestRow>> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.rows.iter().find(|row| row.id == id).cloned())
    }

    pub fn recent(&self, limit: i64) -> Result<Vec<RequestRow>> {
        let inner = self.inner.lock().unwrap();
        let mut rows: Vec<RequestRow> = inner.rows.iter().rev().take(limit.max(1) as usize).cloned().collect();
        rows.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(rows)
    }

    pub fn stats(&self) -> Result<Stats> {
        Ok(self.inner.lock().unwrap().stats.clone())
    }

    pub fn by_model(&self) -> Result<Vec<ModelUsage>> {
        let inner = self.inner.lock().unwrap();
        let mut out: Vec<ModelUsage> = Vec::new();
        for row in inner.rows.iter() {
            let entry = match out.iter_mut().find(|m| m.model == row.model) {
                Some(entry) => entry,
                None => {
                    out.push(ModelUsage { model: row.model.clone(), real_model: row.real_model.clone(), ..Default::default() });
                    out.last_mut().unwrap()
                }
            };
            entry.requests += 1;
            let prompt = row.prompt_tokens.unwrap_or(0);
            let completion = row.completion_tokens.unwrap_or(0);
            let cached = row.cached_tokens.unwrap_or(0);
            entry.prompt_tokens += prompt;
            entry.completion_tokens += completion;
            entry.cached_tokens += cached;
            entry.reasoning_tokens += row.reasoning_tokens.unwrap_or(0);
            entry.total_tokens += row.total_tokens.unwrap_or(prompt + completion);
            if cached > 0 {
                entry.cache_hits += 1;
            }
        }
        for entry in out.iter_mut() {
            entry.credits = credits_of(entry.total_tokens);
        }
        out.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
        Ok(out)
    }

    pub fn by_account(&self) -> Result<Vec<AccountUsage>> {
        let inner = self.inner.lock().unwrap();
        let mut out: Vec<AccountUsage> = Vec::new();
        for row in inner.rows.iter() {
            let entry = match out.iter_mut().find(|a| a.account_id == row.account_id) {
                Some(entry) => entry,
                None => {
                    out.push(AccountUsage {
                        account_id: row.account_id.clone(),
                        account_label: row.account_label.clone(),
                        ..Default::default()
                    });
                    out.last_mut().unwrap()
                }
            };
            entry.requests += 1;
            entry.total_tokens += row
                .total_tokens
                .unwrap_or_else(|| row.prompt_tokens.unwrap_or(0) + row.completion_tokens.unwrap_or(0));
        }
        for entry in out.iter_mut() {
            entry.credits = credits_of(entry.total_tokens);
        }
        out.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
        Ok(out)
    }

    pub fn clear(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.rows.clear();
        inner.stats = Stats::default();
        inner.next_id = 1;
        std::fs::write(&self.path, "")?;
        let _ = std::fs::remove_dir_all(&self.bodies);
        std::fs::create_dir_all(&self.bodies)?;
        Ok(())
    }
}
