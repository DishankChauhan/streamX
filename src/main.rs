use tracing::info;
use anyhow::Result;

mod rtmp;

use rtmp::RtmpServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Starting StreamX RTMP server");

    let rtmp_server = RtmpServer::new(1935);
    
    info!("RTMP server starting on port 1935");
    info!("Connect with: rtmp://localhost:1935/live/STREAM_KEY");

    rtmp_server.start().await?;

    Ok(())
} 