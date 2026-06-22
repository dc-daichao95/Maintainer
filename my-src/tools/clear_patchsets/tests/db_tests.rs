use clear_patchsets::db::Database;
use anyhow::Result;

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
async fn test_get_patchset_status() -> Result<()> {
    let db_path = ":memory:";
    let db = setup_db(db_path).await?;

    db.conn.execute_batch("INSERT INTO patchsets (id, status) VALUES (1, 'Pending');").await?;

    let status = db.get_patchset_status(1).await?;
    assert_eq!(status, Some("Pending".to_string()));

    let status_none = db.get_patchset_status(999).await?;
    assert_eq!(status_none, None);

    Ok(())
}

#[tokio::test]
async fn test_delete_patchset_cascade() -> Result<()> {
    let db_path = ":memory:";
    let db = setup_db(db_path).await?;

    // Insert data for patchset 1
    db.conn.execute_batch(
        "
        INSERT INTO patchsets (id, status) VALUES (1, 'Failed');
        INSERT INTO patchsets_subsystems (patchset_id) VALUES (1);
        
        INSERT INTO patches (id, patchset_id) VALUES (10, 1);
        INSERT INTO patches (id, patchset_id) VALUES (11, 1);
        INSERT INTO patches_subsystems (patch_id) VALUES (10);
        
        INSERT INTO ai_interactions (id) VALUES (100);
        INSERT INTO reviews (id, patchset_id, interaction_id) VALUES (20, 1, 100);
        
        INSERT INTO findings (id, review_id) VALUES (30, 20);
        INSERT INTO tool_usages (id, review_id) VALUES (40, 20);
        
        INSERT INTO email_outbox (id, patch_id) VALUES (50, 10);
        
        INSERT INTO messages (id, content) VALUES (1, 'keep me');
        "
    ).await?;

    // Insert data for patchset 2 (should not be deleted)
    db.conn.execute_batch(
        "
        INSERT INTO patchsets (id, status) VALUES (2, 'Pending');
        INSERT INTO patches (id, patchset_id) VALUES (12, 2);
        "
    ).await?;

    // Execute delete
    db.delete_patchset(1).await?;

    // Verify deletion
    assert_eq!(count(&db, "patchsets", "id = 1").await?, 0);
    assert_eq!(count(&db, "patchsets_subsystems", "patchset_id = 1").await?, 0);
    assert_eq!(count(&db, "patches", "patchset_id = 1").await?, 0);
    assert_eq!(count(&db, "patches_subsystems", "patch_id = 10").await?, 0);
    assert_eq!(count(&db, "reviews", "patchset_id = 1").await?, 0);
    assert_eq!(count(&db, "ai_interactions", "id = 100").await?, 0);
    assert_eq!(count(&db, "findings", "review_id = 20").await?, 0);
    assert_eq!(count(&db, "tool_usages", "review_id = 20").await?, 0);
    assert_eq!(count(&db, "email_outbox", "patch_id = 10").await?, 0);

    // Verify other data is kept
    assert_eq!(count(&db, "patchsets", "id = 2").await?, 1);
    assert_eq!(count(&db, "patches", "patchset_id = 2").await?, 1);
    assert_eq!(count(&db, "messages", "id = 1").await?, 1);

    Ok(())
}
