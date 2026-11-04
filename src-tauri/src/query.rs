//! Reads for the WebView.
//!
//! The WebView has no access to tauri-plugin-sql (whose `select` would run
//! any statement, writes included). It reads through this command instead,
//! which uses a connection opened with SQLite's read-only flag, so a write
//! fails in SQLite itself whatever SQL is sent. Writes go through the
//! permission-checked commands in the other modules.

use serde::Deserialize;
use serde_json::{Map, Value};
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::ctx::Ctx;
use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
pub struct SelectArgs {
    pub sql: String,
    #[serde(default)]
    pub params: Vec<Value>,
}

/// Runs a query on the read-only pool and returns rows as JSON objects.
pub async fn select(ctx: &Ctx, args: SelectArgs) -> Result<Vec<Map<String, Value>>> {
    let (sql, params) = (args.sql.as_str(), &args.params);
    let head = sql.trim_start().get(..6).unwrap_or_default().to_ascii_uppercase();
    if !(head.starts_with("SELECT") || head.starts_with("WITH")) {
        return Err(Error::Invalid("the bridge only runs SELECT queries".into()));
    }
    let mut q = sqlx::query(sql);
    for p in params {
        q = match p {
            Value::Null => q.bind(None::<String>),
            Value::Bool(b) => q.bind(*b),
            Value::Number(n) => match n.as_i64() {
                Some(i) => q.bind(i),
                None => q.bind(n.as_f64()),
            },
            Value::String(s) => q.bind(s.clone()),
            other => q.bind(other.to_string()),
        };
    }
    let rows = q.fetch_all(&ctx.read_pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(i)?;
            let value = if raw.is_null() {
                Value::Null
            } else {
                match raw.type_info().name() {
                    "INTEGER" | "BOOLEAN" => Value::from(row.try_get::<i64, _>(i)?),
                    "REAL" => Value::from(row.try_get::<f64, _>(i)?),
                    "BLOB" => Value::Null,
                    _ => Value::from(row.try_get::<String, _>(i)?),
                }
            };
            obj.insert(col.name().to_owned(), value);
        }
        out.push(obj);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::test_support;

    #[test]
    fn read_pool_refuses_writes() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx_with_read_only_pool().await;
            let rows = select(&ctx, SelectArgs {
                sql: "SELECT key, value FROM settings WHERE key = $1".into(),
                params: vec![Value::from("language")],
            })
            .await
            .unwrap();
            assert_eq!(rows[0]["value"], "ku");

            // Rejected up front…
            assert!(select(&ctx, SelectArgs { sql: "DELETE FROM settings".into(), params: vec![] }).await.is_err());
            // …and by SQLite when disguised as a SELECT
            let sneaky = select(&ctx, SelectArgs {
                sql: "WITH x AS (SELECT 1) DELETE FROM settings".into(),
                params: vec![],
            })
            .await;
            assert!(sneaky.is_err());
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM settings").fetch_one(&ctx.pool).await.unwrap();
            assert!(n > 0, "nothing was deleted");
        });
    }
}
