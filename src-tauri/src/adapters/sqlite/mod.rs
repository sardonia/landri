use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use rusqlite::{params, Connection};
use tokio::task;

use crate::{
    domain::{HistoryItemDetail, HistoryItemSummary, InstalledModel},
    error::{AppError, AppResult},
    ports::{history::HistoryPort, registry::ModelRegistryPort},
};

#[derive(Clone)]
pub struct SqliteStore {
    db_path: Arc<PathBuf>,
}

impl SqliteStore {
    pub fn new(db_path: PathBuf) -> AppResult<Self> {
        let store = Self {
            db_path: Arc::new(db_path),
        };
        store.init_blocking()?;
        Ok(store)
    }

    fn init_blocking(&self) -> AppResult<()> {
        let conn = Connection::open(self.db_path.as_ref())
            .map_err(|e| AppError::Db(e.to_string()))?;

        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS models (
                model_key TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                local_path TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                verified INTEGER NOT NULL,
                installed_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                model_key TEXT NOT NULL,
                repo_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                prompt TEXT NOT NULL,
                output TEXT NOT NULL,
                params_json TEXT NOT NULL,
                prompt_tokens INTEGER,
                completion_tokens INTEGER,
                total_tokens INTEGER
            );
            "#,
        )
        .map_err(|e| AppError::Db(e.to_string()))?;

        Ok(())
    }

    fn with_conn<T, F>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&Connection) -> AppResult<T>,
    {
        let conn = Connection::open(self.db_path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
        f(&conn)
    }
}

#[async_trait]
impl ModelRegistryPort for SqliteStore {
    async fn list_installed(&self) -> AppResult<Vec<InstalledModel>> {
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "SELECT model_key, repo_id, filename, local_path, size_bytes, verified, installed_at FROM models ORDER BY installed_at DESC",
                )
                .map_err(|e| AppError::Db(e.to_string()))?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(InstalledModel {
                        model_key: row.get(0)?,
                        repo_id: row.get(1)?,
                        filename: row.get(2)?,
                        local_path: row.get(3)?,
                        size_bytes: row.get::<_, i64>(4)? as u64,
                        verified: row.get::<_, i64>(5)? != 0,
                        installed_at: row.get(6)?,
                    })
                })
                .map_err(|e| AppError::Db(e.to_string()))?;

            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| AppError::Db(e.to_string()))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }

    async fn get_installed(&self, model_key: &str) -> AppResult<Option<InstalledModel>> {
        let model_key = model_key.to_string();
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "SELECT model_key, repo_id, filename, local_path, size_bytes, verified, installed_at FROM models WHERE model_key = ?1",
                )
                .map_err(|e| AppError::Db(e.to_string()))?;

            let mut rows = stmt
                .query(params![model_key])
                .map_err(|e| AppError::Db(e.to_string()))?;

            if let Some(row) = rows.next().map_err(|e| AppError::Db(e.to_string()))? {
                Ok(Some(InstalledModel {
                    model_key: row.get(0).map_err(|e| AppError::Db(e.to_string()))?,
                    repo_id: row.get(1).map_err(|e| AppError::Db(e.to_string()))?,
                    filename: row.get(2).map_err(|e| AppError::Db(e.to_string()))?,
                    local_path: row.get(3).map_err(|e| AppError::Db(e.to_string()))?,
                    size_bytes: row.get::<_, i64>(4).map_err(|e| AppError::Db(e.to_string()))? as u64,
                    verified: row.get::<_, i64>(5).map_err(|e| AppError::Db(e.to_string()))? != 0,
                    installed_at: row.get(6).map_err(|e| AppError::Db(e.to_string()))?,
                }))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }

    async fn upsert_installed(&self, model: &InstalledModel) -> AppResult<()> {
        let model = model.clone();
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            conn.execute(
                "INSERT INTO models (model_key, repo_id, filename, local_path, size_bytes, verified, installed_at)\
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)\
                 ON CONFLICT(model_key) DO UPDATE SET\
                   repo_id=excluded.repo_id, filename=excluded.filename, local_path=excluded.local_path,\
                   size_bytes=excluded.size_bytes, verified=excluded.verified, installed_at=excluded.installed_at",
                params![
                    model.model_key,
                    model.repo_id,
                    model.filename,
                    model.local_path,
                    model.size_bytes as i64,
                    if model.verified { 1 } else { 0 },
                    model.installed_at,
                ],
            )
            .map_err(|e| AppError::Db(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }

    async fn delete_installed(&self, model_key: &str) -> AppResult<()> {
        let model_key = model_key.to_string();
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            conn.execute("DELETE FROM models WHERE model_key = ?1", params![model_key])
                .map_err(|e| AppError::Db(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }
}

#[async_trait]
impl HistoryPort for SqliteStore {
    async fn list_history(&self, limit: usize) -> AppResult<Vec<HistoryItemSummary>> {
        let limit = limit as i64;
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, created_at, model_key, repo_id, filename, prompt, output, total_tokens\
                     FROM history ORDER BY created_at DESC LIMIT ?1",
                )
                .map_err(|e| AppError::Db(e.to_string()))?;

            let rows = stmt
                .query_map(params![limit], |row| {
                    let prompt: String = row.get(5)?;
                    let output: String = row.get(6)?;
                    Ok(HistoryItemSummary {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        model_key: row.get(2)?,
                        prompt_preview: truncate_preview(&prompt, 120),
                        output_preview: truncate_preview(&output, 140),
                        total_tokens: row.get::<_, Option<i64>>(7)?.map(|v| v as u32),
                    })
                })
                .map_err(|e| AppError::Db(e.to_string()))?;

            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| AppError::Db(e.to_string()))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }

    async fn get_history(&self, id: &str) -> AppResult<Option<HistoryItemDetail>> {
        let id = id.to_string();
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, created_at, model_key, repo_id, filename, prompt, output, params_json, prompt_tokens, completion_tokens, total_tokens\
                     FROM history WHERE id = ?1",
                )
                .map_err(|e| AppError::Db(e.to_string()))?;

            let mut rows = stmt
                .query(params![id])
                .map_err(|e| AppError::Db(e.to_string()))?;

            if let Some(row) = rows.next().map_err(|e| AppError::Db(e.to_string()))? {
                Ok(Some(HistoryItemDetail {
                    id: row.get(0).map_err(|e| AppError::Db(e.to_string()))?,
                    created_at: row.get(1).map_err(|e| AppError::Db(e.to_string()))?,
                    model_key: row.get(2).map_err(|e| AppError::Db(e.to_string()))?,
                    repo_id: row.get(3).map_err(|e| AppError::Db(e.to_string()))?,
                    filename: row.get(4).map_err(|e| AppError::Db(e.to_string()))?,
                    prompt: row.get(5).map_err(|e| AppError::Db(e.to_string()))?,
                    output: row.get(6).map_err(|e| AppError::Db(e.to_string()))?,
                    params_json: row.get(7).map_err(|e| AppError::Db(e.to_string()))?,
                    prompt_tokens: row.get::<_, Option<i64>>(8).map_err(|e| AppError::Db(e.to_string()))?.map(|v| v as u32),
                    completion_tokens: row.get::<_, Option<i64>>(9).map_err(|e| AppError::Db(e.to_string()))?.map(|v| v as u32),
                    total_tokens: row.get::<_, Option<i64>>(10).map_err(|e| AppError::Db(e.to_string()))?.map(|v| v as u32),
                }))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }

    async fn insert_history(&self, item: &HistoryItemDetail) -> AppResult<()> {
        let item = item.clone();
        let path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            conn.execute(
                "INSERT INTO history (id, created_at, model_key, repo_id, filename, prompt, output, params_json, prompt_tokens, completion_tokens, total_tokens)\
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    item.id,
                    item.created_at,
                    item.model_key,
                    item.repo_id,
                    item.filename,
                    item.prompt,
                    item.output,
                    item.params_json,
                    item.prompt_tokens.map(|v| v as i64),
                    item.completion_tokens.map(|v| v as i64),
                    item.total_tokens.map(|v| v as i64),
                ],
            )
            .map_err(|e| AppError::Db(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }


    async fn clear_history(&self) -> AppResult<()> {
        let path = self.db_path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(path.as_ref()).map_err(|e| AppError::Db(e.to_string()))?;
            conn.execute("DELETE FROM history", []).map_err(|e| AppError::Db(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}