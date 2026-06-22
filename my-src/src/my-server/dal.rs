use crate::models::ServerConfig;
use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};

/// Repository trait for managing ServerConfig entities.
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait ServerRepository: Send + Sync {
    async fn list_servers(&self) -> Result<Vec<ServerConfig>>;
    async fn get_server(&self, id: i64) -> Result<Option<ServerConfig>>;
    async fn create_server(&self, server: &ServerConfig) -> Result<i64>;
    async fn update_server(&self, id: i64, server: &ServerConfig) -> Result<()>;
    async fn delete_server(&self, id: i64) -> Result<()>;
}

/// SQLite implementation of ServerRepository.
#[derive(Clone)]
pub struct SqliteRepository {
    pool: Pool<Sqlite>,
}

impl SqliteRepository {
    pub async fn new(db_url: &str) -> Result<Self> {
        if !db_url.starts_with("sqlite::memory:") && !std::path::Path::new(db_url.trim_start_matches("sqlite:")).exists() {
            std::fs::File::create(db_url.trim_start_matches("sqlite:"))?;
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(db_url)
            .await?;

        // Initialize the table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                ip TEXT NOT NULL,
                web_port INTEGER NOT NULL,
                description TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl ServerRepository for SqliteRepository {
    async fn list_servers(&self) -> Result<Vec<ServerConfig>> {
        let servers = sqlx::query_as::<_, ServerConfig>(
            "SELECT id, name, ip, web_port, description FROM servers",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(servers)
    }

    async fn get_server(&self, id: i64) -> Result<Option<ServerConfig>> {
        let server = sqlx::query_as::<_, ServerConfig>(
            "SELECT id, name, ip, web_port, description FROM servers WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(server)
    }

    async fn create_server(&self, server: &ServerConfig) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO servers (name, ip, web_port, description) VALUES (?, ?, ?, ?)",
        )
        .bind(&server.name)
        .bind(&server.ip)
        .bind(server.web_port)
        .bind(&server.description)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    async fn update_server(&self, id: i64, server: &ServerConfig) -> Result<()> {
        sqlx::query(
            "UPDATE servers SET name = ?, ip = ?, web_port = ?, description = ? WHERE id = ?",
        )
        .bind(&server.name)
        .bind(&server.ip)
        .bind(server.web_port)
        .bind(&server.description)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_server(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM servers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_repository() -> Result<()> {
        let repo = SqliteRepository::new("sqlite::memory:").await?;

        // Create
        let server = ServerConfig {
            id: None,
            name: "Test Server".into(),
            ip: "127.0.0.1".into(),
            web_port: 8080,
            description: "A test server".into(),
        };
        let id = repo.create_server(&server).await?;
        assert!(id > 0);

        // Get
        let fetched = repo.get_server(id).await?.unwrap();
        assert_eq!(fetched.name, "Test Server");
        assert_eq!(fetched.ip, "127.0.0.1");
        assert_eq!(fetched.web_port, 8080);
        assert_eq!(fetched.description, "A test server");

        // List
        let list = repo.list_servers().await?;
        assert_eq!(list.len(), 1);

        // Update
        let mut updated_server = fetched.clone();
        updated_server.name = "Updated Server".into();
        repo.update_server(id, &updated_server).await?;

        let fetched_updated = repo.get_server(id).await?.unwrap();
        assert_eq!(fetched_updated.name, "Updated Server");

        // Delete
        repo.delete_server(id).await?;
        let list_after_delete = repo.list_servers().await?;
        assert_eq!(list_after_delete.len(), 0);

        Ok(())
    }
}
