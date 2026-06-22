use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Represents a remote Sashiko server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerConfig {
    pub id: Option<i64>,
    pub name: String,
    pub ip: String,
    pub web_port: u16,
    pub description: String,
}
