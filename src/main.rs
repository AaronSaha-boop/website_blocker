use website_blocker::config::config::Config;
use website_blocker::daemon;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let handle: daemon::DaemonHandle = daemon::run(config).await?;
    
    tokio::signal::ctrl_c().await?;
    handle.shutdown(false).await?;
    Ok(())
}