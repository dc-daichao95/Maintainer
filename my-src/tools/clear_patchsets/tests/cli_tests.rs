use anyhow::Result;
use std::process::Command;
use clear_patchsets::db::Database;
use tempfile::NamedTempFile;

async fn setup_db(db_path: &str) -> Result<Database> {
    let db = Database::new(db_path).await?;
    
    // Create tables
    db.conn.execute_batch(
        "
        CREATE TABLE patchsets (id INTEGER PRIMARY KEY, status TEXT);
        CREATE TABLE patchsets_subsystems (patchset_id INTEGER);
        CREATE TABLE patches (id INTEGER PRIMARY KEY, patchset_id INTEGER);
        CREATE TABLE patches_subsystems (patch_id INTEGER);
        CREATE TABLE reviews (id INTEGER PRIMARY KEY, patchset_id INTEGER, interaction_id INTEGER);
        CREATE TABLE findings (id INTEGER PRIMARY KEY, review_id INTEGER);
        CREATE TABLE tool_usages (id INTEGER PRIMARY KEY, review_id INTEGER);
        CREATE TABLE email_outbox (id INTEGER PRIMARY KEY, patch_id INTEGER);
        CREATE TABLE ai_interactions (id INTEGER PRIMARY KEY);
        CREATE TABLE messages (id INTEGER PRIMARY KEY, content TEXT);
        "
    ).await?;

    Ok(db)
}

async fn count(db: &Database, table: &str, condition: &str) -> Result<i64> {
    let mut rows = db.conn.query(&format!("SELECT COUNT(*) FROM {} WHERE {}", table, condition), ()).await?;
    let row = rows.next().await?.unwrap();
    Ok(row.get::<i64>(0)?)
}

#[tokio::test]
async fn test_cli_rejects_in_review() -> Result<()> {
    let temp_db = NamedTempFile::new()?;
    let db_path = temp_db.path().to_str().unwrap();
    
    let db = setup_db(db_path).await?;
    db.conn.execute_batch("INSERT INTO patchsets (id, status) VALUES (1, 'In Review');").await?;

    // Run CLI
    let output = Command::new(env!("CARGO_BIN_EXE_clear_patchsets"))
        .arg("--db")
        .arg(db_path)
        .arg("1")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    assert!(stdout.contains("Cannot delete patchset 1") || stderr.contains("Cannot delete patchset 1") || stdout.contains("warning") || stderr.contains("warning"));
    
    // Verify not deleted
    assert_eq!(count(&db, "patchsets", "id = 1").await?, 1);

    Ok(())
}

#[tokio::test]
async fn test_cli_success_delete() -> Result<()> {
    let temp_db = NamedTempFile::new()?;
    let db_path = temp_db.path().to_str().unwrap();
    
    let db = setup_db(db_path).await?;
    db.conn.execute_batch(
        "
        INSERT INTO patchsets (id, status) VALUES (2, 'Failed');
        INSERT INTO patches (id, patchset_id) VALUES (10, 2);
        "
    ).await?;

    // Run CLI
    let output = Command::new(env!("CARGO_BIN_EXE_clear_patchsets"))
        .arg("--db")
        .arg(db_path)
        .arg("2")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(stdout.contains("Deleted patchset 2") || stdout.contains("Successfully"));
    
    // Verify deleted
    assert_eq!(count(&db, "patchsets", "id = 2").await?, 0);
    assert_eq!(count(&db, "patches", "patchset_id = 2").await?, 0);

    Ok(())
}
