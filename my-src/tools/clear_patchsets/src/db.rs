use anyhow::Result;

pub struct Database {
    pub conn: libsql::Connection,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self> {
        let db = libsql::Builder::new_local(db_path).build().await?;
        let conn = db.connect()?;
        Ok(Self { conn })
    }

    pub async fn get_patchset_status(&self, id: i64) -> Result<Option<String>> {
        let mut rows = self.conn.query("SELECT status FROM patchsets WHERE id = ?", [id]).await?;
        if let Some(row) = rows.next().await? {
            let status: String = row.get(0)?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_patchset(&self, id: i64) -> Result<()> {
        let tx = self.conn.transaction().await?;
        
        // 1. email_outbox (depends on patches)
        tx.execute(
            "DELETE FROM email_outbox WHERE patch_id IN (SELECT id FROM patches WHERE patchset_id = ?)",
            [id],
        ).await?;
        
        // 2. tool_usages (depends on reviews)
        tx.execute(
            "DELETE FROM tool_usages WHERE review_id IN (SELECT id FROM reviews WHERE patchset_id = ?)",
            [id],
        ).await?;
        
        // 3. findings (depends on reviews)
        tx.execute(
            "DELETE FROM findings WHERE review_id IN (SELECT id FROM reviews WHERE patchset_id = ?)",
            [id],
        ).await?;
        
        // 4. ai_interactions (referenced by reviews)
        tx.execute(
            "DELETE FROM ai_interactions WHERE id IN (SELECT interaction_id FROM reviews WHERE patchset_id = ?)",
            [id],
        ).await?;
        
        // 5. reviews (depends on patchsets)
        tx.execute(
            "DELETE FROM reviews WHERE patchset_id = ?",
            [id],
        ).await?;
        
        // 6. patches_subsystems (depends on patches)
        tx.execute(
            "DELETE FROM patches_subsystems WHERE patch_id IN (SELECT id FROM patches WHERE patchset_id = ?)",
            [id],
        ).await?;
        
        // 7. patches (depends on patchsets)
        tx.execute(
            "DELETE FROM patches WHERE patchset_id = ?",
            [id],
        ).await?;
        
        // 8. patchsets_subsystems (depends on patchsets)
        tx.execute(
            "DELETE FROM patchsets_subsystems WHERE patchset_id = ?",
            [id],
        ).await?;
        
        // 9. patchsets (main table)
        tx.execute(
            "DELETE FROM patchsets WHERE id = ?",
            [id],
        ).await?;
        
        tx.commit().await?;
        Ok(())
    }
}
