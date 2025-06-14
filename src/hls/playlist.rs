use crate::config::Config;
use std::{collections::VecDeque, path::PathBuf};
use tokio::fs;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct Segment {
    pub filename: String,
    pub duration: f64,
    pub sequence: u64,
}

#[derive(Debug)]
pub struct PlaylistManager {
    config: Config,
    stream_key: String,
    segments: VecDeque<Segment>,
    sequence_number: u64,
    target_duration: u32,
}

impl PlaylistManager {
    pub async fn new(config: Config, stream_key: String) -> crate::error::Result<Self> {
        Ok(Self {
            config,
            stream_key,
            segments: VecDeque::new(),
            sequence_number: 0,
            target_duration: 10, // Default target duration
        })
    }

    pub async fn update(&mut self) -> crate::error::Result<()> {
        let stream_dir = self.config.stream_dir(&self.stream_key);
        let playlist_path = stream_dir.join("playlist.m3u8");

        // Check if FFmpeg-generated playlist exists
        if !playlist_path.exists() {
            debug!("FFmpeg playlist not yet available for stream: {}", self.stream_key);
            return Ok(());
        }

        // Read and parse the FFmpeg-generated playlist
        match self.parse_ffmpeg_playlist(&playlist_path).await {
            Ok(_) => {
                debug!("Successfully updated playlist for stream: {}", self.stream_key);
            }
            Err(e) => {
                warn!("Failed to parse FFmpeg playlist: {}", e);
            }
        }

        Ok(())
    }

    async fn parse_ffmpeg_playlist(&mut self, playlist_path: &PathBuf) -> crate::error::Result<()> {
        let content = fs::read_to_string(playlist_path).await?;
        let lines: Vec<&str> = content.lines().collect();

        let mut new_segments = VecDeque::new();
        let mut current_duration = 0.0;
        let mut sequence = self.sequence_number;

        for line in lines.iter() {
            if line.starts_with("#EXT-X-TARGETDURATION:") {
                if let Ok(duration) = line.split(':').nth(1).unwrap_or("10").parse::<u32>() {
                    self.target_duration = duration;
                }
            } else if line.starts_with("#EXTINF:") {
                // Parse segment duration
                if let Some(duration_str) = line.strip_prefix("#EXTINF:") {
                    if let Some(duration_part) = duration_str.split(',').next() {
                        current_duration = duration_part.parse().unwrap_or(self.config.segment_duration as f64);
                    }
                }
            } else if line.ends_with(".ts") && !line.starts_with('#') {
                // This is a segment file
                let segment = Segment {
                    filename: line.to_string(),
                    duration: current_duration,
                    sequence,
                };
                new_segments.push_back(segment);
                sequence += 1;
                current_duration = 0.0;
            }
        }

        // Update our segment list
        self.segments = new_segments;
        if sequence > self.sequence_number {
            self.sequence_number = sequence;
        }

        Ok(())
    }

    pub async fn get_content(&self) -> crate::error::Result<String> {
        if self.segments.is_empty() {
            return Ok(self.generate_empty_playlist());
        }

        let mut playlist = String::new();
        
        // Header
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:3\n");
        playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", self.target_duration));
        
        // Sequence number (use the sequence of the first segment)
        if let Some(first_segment) = self.segments.front() {
            playlist.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", first_segment.sequence));
        }

        // Segments
        for segment in &self.segments {
            playlist.push_str(&format!("#EXTINF:{:.3},\n", segment.duration));
            playlist.push_str(&format!("{}\n", segment.filename));
        }

        Ok(playlist)
    }

    fn generate_empty_playlist(&self) -> String {
        format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:0\n",
            self.target_duration
        )
    }

    pub fn get_segments(&self) -> &VecDeque<Segment> {
        &self.segments
    }

    pub fn is_live(&self) -> bool {
        !self.segments.is_empty()
    }
} 