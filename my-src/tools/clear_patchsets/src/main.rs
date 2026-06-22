use clap::Parser;
use anyhow::Result;
use clear_patchsets::db::Database;

#[derive(Parser)]
#[command(name = "clear_patchsets", about = "Clear unreviewed patchsets and their associated data")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(short, long, default_value = "sashiko.db")]
    db: String,

    /// The IDs of the patchsets to clear
    ids: Vec<i64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let db = Database::new(&cli.db).await?;

    for id in cli.ids {
        match db.get_patchset_status(id).await? {
            Some(status) => {
                if status == "In Review" || status == "Reviewed" {
                    println!("Warning: Cannot delete patchset {} because its status is '{}'", id, status);
                } else {
                    db.delete_patchset(id).await?;
                    println!("Successfully deleted patchset {}", id);
                }
            }
            None => {
                println!("Warning: Patchset {} not found", id);
            }
        }
    }

    Ok(())
}
