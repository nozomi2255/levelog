use std::path::Path;

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_millis(5_000));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&pool)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[tokio::test]
    async fn migration_enables_catalog_and_foreign_keys() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).await.unwrap();
        let skills: i64 = sqlx::query("SELECT COUNT(*) count FROM skills")
            .fetch_one(db.pool())
            .await
            .unwrap()
            .get("count");
        assert_eq!(skills, 15);
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(db.pool())
            .await
            .unwrap()
            .get(0);
        assert_eq!(fk, 1);
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");
    }

    #[tokio::test]
    async fn persisted_data_survives_pool_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("restart.db");
        let db = Database::open(&path).await.unwrap();
        sqlx::query("INSERT INTO activities (id, occurred_on, action_text, challenge_text, outcome_text, created_at) VALUES ('restart-activity', '2026-07-20', '原文', '', '', '2026-07-20T00:00:00.000Z')")
            .execute(db.pool())
            .await
            .unwrap();
        db.pool().close().await;
        let reopened = Database::open(&path).await.unwrap();
        let text: String =
            sqlx::query_scalar("SELECT action_text FROM activities WHERE id = 'restart-activity'")
                .fetch_one(reopened.pool())
                .await
                .unwrap();
        assert_eq!(text, "原文");
    }
}
