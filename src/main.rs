mod service;

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::transport::stdio;
use service::ObsidianService;

#[tokio::main]
async fn main() -> Result<()> {
    let vault_location = std::env::var("VAULT_LOCATION")
        .context("VAULT_LOCATION environment variable must be set")?;
    
    let daily_notes_path = std::env::var("DAILY_NOTES_PATH").ok();
    let weekly_notes_path = std::env::var("WEEKLY_NOTES_PATH").ok();
    let monthly_notes_path = std::env::var("MONTHLY_NOTES_PATH").ok();
    let templates_path = std::env::var("TEMPLATES_PATH").ok();

    let service = ObsidianService::new(
        &vault_location,
        daily_notes_path.as_deref(),
        weekly_notes_path.as_deref(),
        monthly_notes_path.as_deref(),
        templates_path.as_deref(),
    )?;

    let server = service.serve(stdio()).await
        .map_err(|e| {
            eprintln!("Error starting server: {:?}", e);
            e
        })?;
    
    server.waiting().await
        .map_err(|e| {
            eprintln!("Error waiting for server: {:?}", e);
            e
        })?;

    Ok(())
}
