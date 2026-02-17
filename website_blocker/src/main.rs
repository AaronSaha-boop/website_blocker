use crate::config::Config;
use crate::daemon;
use focusd::daemon;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let handle: DaemonHandle = daemon::run(config).await?;
    
    // Block forever (launchd manages lifecycle)
    tokio::signal::ctrl_c().await?;
    handle.shutdown(false).await?;
    
    Ok(())
}