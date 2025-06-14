# StreamX - Real Time Video Streaming Server

🎥 A low-latency streaming server built in Rust that accepts RTMP streams and serves them as HLS to viewers.

## Phase 1 Features (Current)

- ✅ **RTMP Server**: Accept live video streams from broadcasters via RTMP
- ✅ **HLS Output**: Convert RTMP streams to HLS format for web playback
- ✅ **HTTP Server**: Serve HLS playlists and segments via HTTP
- ✅ **Web Interface**: Beautiful web UI for viewing streams
- ✅ **Multiple Streams**: Support for concurrent streams
- ✅ **Auto-Discovery**: Automatic detection of active streams

## Prerequisites

### System Requirements
- **Rust** (latest stable version)
- **FFmpeg** installed and available in PATH
- **macOS/Linux/Windows** (tested on macOS)

### Installing FFmpeg

**macOS (using Homebrew):**
```bash
brew install ffmpeg
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install ffmpeg
```

**Windows:**
Download from [FFmpeg official website](https://ffmpeg.org/download.html) and add to PATH.

## Quick Start

### 1. Clone and Build
```bash
git clone https://github.com/DishankChauhan/streamX
cd streamx
cargo build --release
```

### 2. Run the Server
```bash
cargo run
```

The server will start with default settings:
- **RTMP Port**: 1935
- **HTTP Port**: 8080
- **Streams Directory**: `./streams`

### 3. Configure Your Streaming Software

**OBS Studio Settings:**
- **Server**: `rtmp://localhost:1935/live`
- **Stream Key**: Any unique identifier (e.g., `mystream`)

### 4. View Your Stream
Open your browser and go to:
- **Dashboard**: http://localhost:8080
- **Direct Stream**: http://localhost:8080/stream/YOUR_STREAM_KEY

## Configuration Options

```bash
# Custom ports and settings
cargo run -- \
  --rtmp-port 1935 \
  --http-port 8080 \
  --streams-dir ./my-streams \
  --max-streams 5 \
  --segment-duration 4 \
  --playlist-size 5
```

### CLI Arguments
- `-r, --rtmp-port`: RTMP server port (default: 1935)
- `-h, --http-port`: HTTP server port (default: 8080)
- `-d, --streams-dir`: Directory for stream files (default: ./streams)
- `-m, --max-streams`: Maximum concurrent streams (default: 10)
- `-s, --segment-duration`: HLS segment duration in seconds (default: 4)
- `-n, --playlist-size`: Number of segments in playlist (default: 5)

## API Endpoints

### Stream Viewing
- `GET /` - Main dashboard
- `GET /stream/{stream_key}` - Stream viewer page
- `GET /stream/{stream_key}/playlist.m3u8` - HLS playlist
- `GET /stream/{stream_key}/{segment}.ts` - HLS segments

### Stream Management
- `GET /streams` - List active streams (JSON)

## Architecture

```
┌─────────────────┐    RTMP     ┌─────────────────┐    HLS      ┌─────────────────┐
│   OBS Studio    │  ──────────►│   StreamX       │  ──────────►│   Web Browser   │
│   (Broadcaster) │             │   Server        │             │   (Viewer)      │
└─────────────────┘             └─────────────────┘             └─────────────────┘
                                        │
                                        ▼
                                 ┌─────────────┐
                                 │   FFmpeg    │
                                 │ (Segmenter) │
                                 └─────────────┘
```

### Components

1. **RTMP Server** (`src/rtmp/`): Handles incoming RTMP connections and handshakes
2. **HLS Processor** (`src/hls/`): Manages FFmpeg processes for stream segmentation
3. **HTTP Server** (`src/http_server.rs`): Serves web interface and HLS content
4. **Configuration** (`src/config.rs`): Centralized configuration management

## Streaming Workflow

1. **Broadcaster** starts streaming to `rtmp://localhost:1935/live/STREAM_KEY`
2. **RTMP Server** accepts the connection and performs handshake
3. **HLS Processor** spawns FFmpeg to segment the stream into `.ts` files
4. **HTTP Server** serves the generated `.m3u8` playlist and `.ts` segments
5. **Viewers** can watch at `http://localhost:8080/stream/STREAM_KEY`

## File Structure

```
streamx/
├── src/
│   ├── main.rs              # Entry point
│   ├── config.rs            # Configuration management
│   ├── error.rs             # Error handling
│   ├── http_server.rs       # HTTP server and web UI
│   ├── rtmp/
│   │   ├── mod.rs           # RTMP server
│   │   ├── protocol.rs      # RTMP protocol definitions
│   │   └── handshake.rs     # RTMP handshake implementation
│   └── hls/
│       ├── mod.rs           # HLS processor
│       └── playlist.rs      # Playlist management
├── streams/                 # Generated stream files (auto-created)
├── Cargo.toml              # Dependencies
└── README.md               # This file
```

## Upcoming Phases

### Phase 2: DASH & Multi-Bitrate
- [ ] DASH streaming support
- [ ] Multiple bitrate renditions
- [ ] Adaptive streaming

### Phase 3: Live Chat & Analytics
- [ ] WebSocket-based live chat
- [ ] Viewer statistics
- [ ] Real-time metrics

### Phase 4: Authentication & Management
- [ ] Stream authentication
- [ ] Web dashboard for management
- [ ] User accounts

### Phase 5: Deployment & Scaling
- [ ] Docker containerization
- [ ] Load balancing support
- [ ] Monitoring integration

## Development

### Running in Development
```bash
# With debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test
```

### Building for Production
```bash
cargo build --release
./target/release/streamx
```

## Troubleshooting

### Common Issues

**"FFmpeg not found"**
- Ensure FFmpeg is installed and in your PATH
- Test with: `ffmpeg -version`

**"Stream not appearing"**
- Check that your streaming software is using the correct RTMP URL
- Verify the stream key matches what you're trying to view
- Check the logs for connection errors

**"Permission denied on port 1935"**
- On some systems, port 1935 requires root privileges
- Try using a different port: `cargo run -- --rtmp-port 19350`

### Logs
StreamX provides detailed logging. Set log level with:
```bash
RUST_LOG=streamx=debug,rtmp=debug cargo run
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Tokio](https://tokio.rs/) for async runtime
- Uses [Warp](https://github.com/seanmonstar/warp) for HTTP server
- Powered by [FFmpeg](https://ffmpeg.org/) for media processing
- Web player uses [Video.js](https://videojs.com/) for HLS playback 