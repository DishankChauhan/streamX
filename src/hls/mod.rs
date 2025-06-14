use crate::{config::Config, error::{Result, StreamError}};
use bytes::Bytes;
use std::{
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};
use tokio::{
    io::{AsyncWriteExt, BufWriter},
    process::{Child, Command},
    sync::{mpsc, Mutex},
    time::{interval, Duration},
};
use tracing::{debug, error, info, warn};

mod playlist;

use playlist::PlaylistManager;

#[derive(Clone)]
pub struct HlsProcessor {
    stream_key: String,
    config: Config,
    playlist_manager: Arc<Mutex<PlaylistManager>>,
    ffmpeg_process: Arc<Mutex<Option<Child>>>,
}

impl HlsProcessor {
    pub async fn new(stream_key: String, config: Config) -> Result<Self> {
        let playlist_manager = PlaylistManager::new(config.clone(), stream_key.clone()).await?;

        Ok(Self {
            stream_key,
            config,
            playlist_manager: Arc::new(Mutex::new(playlist_manager)),
            ffmpeg_process: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn process_stream(&self, mut data_receiver: mpsc::UnboundedReceiver<Bytes>) -> Result<()> {
        info!("Starting HLS processing for stream: {}", self.stream_key);

        // Start FFmpeg process for HLS segmentation
        let ffmpeg_child = self.start_ffmpeg_process().await?;
        
        // Store the FFmpeg process
        *self.ffmpeg_process.lock().await = Some(ffmpeg_child);

        // Get stdin handle for writing stream data
        let stdin = self.ffmpeg_process.lock().await
            .as_mut()
            .and_then(|child| child.stdin.take())
            .ok_or_else(|| StreamError::Ffmpeg("Failed to get FFmpeg stdin".to_string()))?;

        let mut stdin_writer = BufWriter::new(stdin);

        // Start playlist update task
        let playlist_updater = self.clone();
        let playlist_task = tokio::spawn(async move {
            playlist_updater.playlist_update_loop().await;
        });

        // Process incoming stream data
        while let Some(data) = data_receiver.recv().await {
            if let Err(e) = stdin_writer.write_all(&data).await {
                error!("Failed to write data to FFmpeg: {}", e);
                break;
            }
            
            if let Err(e) = stdin_writer.flush().await {
                error!("Failed to flush data to FFmpeg: {}", e);
                break;
            }
        }

        info!("Stream ended for: {}", self.stream_key);

        // Clean up
        if let Some(mut child) = self.ffmpeg_process.lock().await.take() {
            let _ = child.kill().await;
        }

        playlist_task.abort();
        Ok(())
    }

    async fn start_ffmpeg_process(&self) -> Result<Child> {
        let stream_dir = self.config.stream_dir(&self.stream_key);
        let segment_pattern = stream_dir.join("segment_%03d.ts");
        let playlist_path = stream_dir.join("playlist.m3u8");

        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-f", "flv",                           // Input format (FLV from RTMP)
            "-i", "pipe:0",                        // Input from stdin
            "-c", "copy",                          // Copy codecs without re-encoding
            "-f", "hls",                           // Output format HLS
            "-hls_time", &self.config.segment_duration.to_string(), // Segment duration
            "-hls_list_size", &self.config.playlist_size.to_string(), // Playlist size
            "-hls_flags", "delete_segments",       // Delete old segments
            "-hls_segment_filename", segment_pattern.to_str().unwrap(),
        ])
        .arg(playlist_path.to_str().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

        debug!("Starting FFmpeg with command: {:?}", cmd);

        let child = cmd.spawn()
            .map_err(|e| StreamError::Ffmpeg(format!("Failed to start FFmpeg: {}", e)))?;

        info!("FFmpeg process started for stream: {}", self.stream_key);
        Ok(child)
    }

    async fn playlist_update_loop(&self) {
        let mut update_interval = interval(Duration::from_secs(2));
        
        loop {
            update_interval.tick().await;
            
            if let Err(e) = self.update_playlist().await {
                warn!("Failed to update playlist: {}", e);
            }
        }
    }

    async fn update_playlist(&self) -> Result<()> {
        // Check if FFmpeg is still running
        if let Some(child) = self.ffmpeg_process.lock().await.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    info!("FFmpeg process exited with status: {}", status);
                    return Ok(());
                }
                Ok(None) => {
                    // Process is still running
                }
                Err(e) => {
                    error!("Error checking FFmpeg status: {}", e);
                    return Err(StreamError::Ffmpeg(format!("FFmpeg status error: {}", e)));
                }
            }
        }

        // Update playlist manager
        self.playlist_manager.lock().await.update().await?;

        Ok(())
    }

    pub async fn get_playlist_content(&self) -> Result<String> {
        self.playlist_manager.lock().await.get_content().await
    }

    pub async fn get_segment_path(&self, segment_name: &str) -> Result<PathBuf> {
        let stream_dir = self.config.stream_dir(&self.stream_key);
        let segment_path = stream_dir.join(segment_name);

        if segment_path.exists() {
            Ok(segment_path)
        } else {
            Err(StreamError::StreamNotFound(format!("Segment not found: {}", segment_name)))
        }
    }
} 