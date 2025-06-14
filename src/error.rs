use thiserror::Error;

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("RTMP protocol error: {0}")]
    Rtmp(String),

    #[error("HLS processing error: {0}")]
    Hls(String),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Maximum streams exceeded")]
    MaxStreamsExceeded,

    #[error("Invalid stream key: {0}")]
    InvalidStreamKey(String),

    #[error("FFmpeg error: {0}")]
    Ffmpeg(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, StreamError>; 