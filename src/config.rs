use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub rtmp_port: u16,
    pub http_port: u16,
    pub streams_dir: PathBuf,
    pub max_streams: usize,
    pub segment_duration: u32,
    pub playlist_size: usize,
}

impl Config {
    pub fn stream_dir(&self, stream_key: &str) -> PathBuf {
        self.streams_dir.join(stream_key)
    }

    pub fn playlist_path(&self, stream_key: &str) -> PathBuf {
        self.stream_dir(stream_key).join("playlist.m3u8")
    }
} 