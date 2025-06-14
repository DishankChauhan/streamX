use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};
use tracing::{info, debug};
use std::io;

type HmacSha256 = Hmac<Sha256>;

pub async fn perform_handshake(stream: &mut TcpStream) -> Result<(), io::Error> {
    // Read C0 (1 byte)
    let mut c0 = [0u8; 1];
    stream.read_exact(&mut c0).await?;
    debug!("Received C0: {:02x?}", c0);
    
    if c0[0] != 3 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid RTMP version"));
    }
    
    // Read C1 (1536 bytes)
    let mut c1 = [0u8; 1536];
    stream.read_exact(&mut c1).await?;
    debug!("Received C1: {} bytes", c1.len());
    
    // Send S0 (1 byte)
    stream.write_all(&[3]).await?;
    info!("Sent S0");
    
    // Generate S1 (1536 bytes)
    let mut s1 = [0u8; 1536];
    
    // S1 format:
    // - 4 bytes: timestamp (can be 0)
    // - 4 bytes: zero
    // - 1528 bytes: random data
    
    let timestamp = 0u32.to_be_bytes();
    s1[0..4].copy_from_slice(&timestamp);
    s1[4..8].copy_from_slice(&[0, 0, 0, 0]);
    
    // Fill with random data (simplified)
    for i in 8..1536 {
        s1[i] = (i % 256) as u8;
    }
    
    stream.write_all(&s1).await?;
    info!("Sent S1");
    
    // Send S2 (echo of C1)
    stream.write_all(&c1).await?;
    info!("Sent S2");
    
    // Read C2 (1536 bytes) - echo of S1
    let mut c2 = [0u8; 1536];
    stream.read_exact(&mut c2).await?;
    debug!("Received C2: {} bytes", c2.len());
    
    info!("RTMP handshake completed successfully");
    Ok(())
} 