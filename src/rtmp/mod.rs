use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error, warn, debug};
use std::io;

mod handshake;
mod protocol;

use handshake::perform_handshake;
use protocol::{RtmpHeader, MessageType, parse_rtmp_connect, create_connect_response, parse_rtmp_publish, create_publish_response, parse_rtmp_createstream, create_createstream_response, parse_command_name, create_generic_response, create_onbwdone_message, parse_checkbw_command, create_checkbw_response, create_onbwcheck_message};

pub struct RtmpServer {
    port: u16,
}

impl RtmpServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(&self) -> Result<(), io::Error> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;
        info!("RTMP server listening on port {}", self.port);

        loop {
            let (socket, addr) = listener.accept().await?;
            info!("New RTMP connection from: {}", addr);

            tokio::spawn(async move {
                if let Err(e) = handle_rtmp_connection(socket).await {
                    error!("RTMP connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_rtmp_connection(mut socket: TcpStream) -> Result<(), io::Error> {
    // Perform RTMP handshake
    info!("Starting RTMP handshake");
    perform_handshake(&mut socket).await?;
    info!("✅ RTMP handshake completed successfully");

    // Send initial control messages as per RTMP spec
    send_initial_control_messages(&mut socket).await?;

    // Main message processing loop
    let mut buffer = vec![0u8; 4096];
    let mut buffer_pos = 0;
    
    loop {
        // Set a timeout for reading to see if more data comes
        let read_result = tokio::time::timeout(
            std::time::Duration::from_secs(5), 
            socket.read(&mut buffer[buffer_pos..])
        ).await;
        
        let bytes_read = match read_result {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                info!("⏰ Read timeout - no more data from client");
                continue; // Keep waiting
            }
        };
        
        if bytes_read == 0 {
            info!("Client disconnected");
            break;
        }

        buffer_pos += bytes_read;
        debug!("Received {} bytes from client, total buffer: {} bytes", bytes_read, buffer_pos);
        debug!("Buffer data: {:02x?}", &buffer[..buffer_pos.min(100)]);

        // Process all complete messages in buffer
        let mut processed = 0;
        let mut message_count = 0;
        const MAX_MESSAGES_PER_READ: usize = 10; // Safety limit to prevent infinite loops
        
        while processed < buffer_pos && message_count < MAX_MESSAGES_PER_READ {
            message_count += 1;
            let remaining = &buffer[processed..buffer_pos];
            debug!("Processing from offset {}, remaining {} bytes", processed, remaining.len());
            
            if let Some((header, header_size)) = RtmpHeader::parse(remaining) {
                debug!("Parsed RTMP header: {:?}", header);
                
                let total_message_size = header_size + header.message_length as usize;
                if remaining.len() < total_message_size {
                    debug!("Incomplete message, need {} more bytes", total_message_size - remaining.len());
                    break; // Wait for more data
                }
                
                match MessageType::from(header.message_type_id) {
                    MessageType::Command => {
                        info!("📞 Received RTMP command message");
                        
                        let payload = &remaining[header_size..header_size + header.message_length as usize];
                        debug!("Command payload ({} bytes): {:02x?}", payload.len(), payload);
                        
                        // Try parsing as connect command first
                        if let Some(connect_cmd) = parse_rtmp_connect(payload) {
                            info!("🎯 Parsed connect command: {:?}", connect_cmd);
                            
                            // Send connect response
                            let response = create_connect_response();
                            let response_chunk = create_command_chunk(&response);
                            
                            debug!("Sending connect response chunk: {} bytes", response_chunk.len());
                            socket.write_all(&response_chunk).await?;
                            socket.flush().await?;
                            info!("✅ Sent connect response to client");
                            
                            // Send Stream Begin user control message
                            send_stream_begin(&mut socket, 0).await?;
                            info!("✅ Sent Stream Begin message");
                            
                            // Send onBWDone message to complete bandwidth negotiation
                            let onbwdone = create_onbwdone_message();
                            let onbwdone_chunk = create_command_chunk(&onbwdone);
                            
                            debug!("Sending onBWDone message: {} bytes", onbwdone_chunk.len());
                            socket.write_all(&onbwdone_chunk).await?;
                            socket.flush().await?;
                            info!("✅ Sent onBWDone message - OBS should proceed now!");
                            
                        } else if let Some(createstream_cmd) = parse_rtmp_createstream(payload) {
                            info!("🎯 Parsed createStream command: {:?}", createstream_cmd);
                            
                            // Send createStream response
                            let response = create_createstream_response(createstream_cmd.transaction_id);
                            let response_chunk = create_command_chunk(&response);
                            
                            debug!("Sending createStream response chunk: {} bytes", response_chunk.len());
                            socket.write_all(&response_chunk).await?;
                            socket.flush().await?;
                            info!("✅ Sent createStream response to client");
                            
                        } else if let Some(publish_cmd) = parse_rtmp_publish(payload) {
                            info!("🎯 Parsed publish command: {:?}", publish_cmd);
                            
                            // Send publish response
                            let response = create_publish_response(&publish_cmd.stream_key);
                            let response_chunk = create_command_chunk(&response);
                            
                            debug!("Sending publish response chunk: {} bytes", response_chunk.len());
                            socket.write_all(&response_chunk).await?;
                            socket.flush().await?;
                            info!("✅ Sent publish response to client - streaming started!");
                            
                        } else {
                            warn!("❌ Failed to parse command (not connect, createStream, or publish)");
                            debug!("Raw command payload: {:02x?}", payload);
                            
                            // Try to at least parse the command name to see what OBS is sending
                            if let Some(command_name) = parse_command_name(payload) {
                                warn!("🔍 Unknown command received: '{}'", command_name);
                                
                                // Handle specific commands
                                match command_name.as_str() {
                                    "_checkbw" => {
                                        if let Some(transaction_id) = parse_checkbw_command(payload) {
                                            info!("🎯 Parsed _checkbw command with transaction ID: {}", transaction_id);
                                            
                                            // Send _checkbw response with bandwidth value
                                            let response = create_checkbw_response(transaction_id);
                                            let response_chunk = create_command_chunk(&response);
                                            
                                            debug!("Sending _checkbw response: {} bytes", response_chunk.len());
                                            socket.write_all(&response_chunk).await?;
                                            socket.flush().await?;
                                            info!("✅ Sent _checkbw response");
                                            
                                            // Send onBWCheck message to complete bandwidth negotiation
                                            let onbwcheck = create_onbwcheck_message();
                                            let onbwcheck_chunk = create_command_chunk(&onbwcheck);
                                            
                                            debug!("Sending onBWCheck message: {} bytes", onbwcheck_chunk.len());
                                            socket.write_all(&onbwcheck_chunk).await?;
                                            socket.flush().await?;
                                            info!("✅ Sent onBWCheck - bandwidth negotiation complete!");
                                            
                                        } else {
                                            warn!("❌ Failed to parse _checkbw transaction ID");
                                            let response = create_generic_response("_checkbw");
                                            let response_chunk = create_command_chunk(&response);
                                            socket.write_all(&response_chunk).await?;
                                            socket.flush().await?;
                                        }
                                    }
                                    _ => {
                                        // Send generic response for other unknown commands
                                        let response = create_generic_response(&command_name);
                                        let response_chunk = create_command_chunk(&response);
                                        
                                        debug!("Sending generic response for '{}': {} bytes", command_name, response_chunk.len());
                                        socket.write_all(&response_chunk).await?;
                                        socket.flush().await?;
                                        info!("✅ Sent generic response for '{}'", command_name);
                                    }
                                }
                            } else {
                                error!("❌ Could not parse command name from payload");
                            }
                        }
                    }
                    MessageType::SetChunkSize => {
                        info!("📏 Received Set Chunk Size message");
                        if header.message_length >= 4 {
                            let payload = &remaining[header_size..header_size + 4];
                            let chunk_size = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                            info!("New chunk size: {}", chunk_size);
                        }
                    }
                    MessageType::WindowAcknowledgementSize => {
                        info!("🪟 Received Window Acknowledgement Size message");
                        
                        // Send acknowledgement back
                        if header.message_length >= 4 {
                            let payload = &remaining[header_size..header_size + 4];
                            let ack_size = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                            info!("Window acknowledgement size: {}", ack_size);
                            
                            // Send acknowledgement message
                            let ack_response = create_control_message(3, &0u32.to_be_bytes());
                            socket.write_all(&ack_response).await?;
                            socket.flush().await?;
                            info!("✅ Sent acknowledgement response");
                        }
                    }
                    MessageType::Audio => {
                        info!("🔊 Received audio data");
                    }
                    MessageType::Video => {
                        info!("📹 Received video data");
                    }
                    _ => {
                        debug!("Received message type: {:?}", MessageType::from(header.message_type_id));
                    }
                }
                
                processed += total_message_size;
                debug!("Processed message, advancing by {} bytes", total_message_size);
            } else {
                debug!("❌ Failed to parse RTMP header, remaining bytes: {}", remaining.len());
                debug!("Raw data: {:02x?}", &remaining[..remaining.len().min(50)]);
                break; // Wait for more data
            }
        }
        
        if message_count >= MAX_MESSAGES_PER_READ {
            warn!("⚠️ Hit message processing limit ({} messages), stopping to prevent infinite loop", MAX_MESSAGES_PER_READ);
        }
        
        // Move unprocessed data to beginning of buffer
        if processed > 0 {
            if processed < buffer_pos {
                buffer.copy_within(processed..buffer_pos, 0);
                buffer_pos -= processed;
                debug!("Moved {} unprocessed bytes to buffer start", buffer_pos);
            } else {
                buffer_pos = 0;
                debug!("All data processed, buffer cleared");
            }
        }
    }

    Ok(())
}

async fn send_initial_control_messages(socket: &mut TcpStream) -> Result<(), io::Error> {
    info!("Sending initial RTMP control messages");

    // 1. Window Acknowledgement Size (5MB)
    let window_ack_size = create_control_message(5, &(5_000_000u32).to_be_bytes());
    socket.write_all(&window_ack_size).await?;
    
    // 2. Set Peer Bandwidth (5MB, Hard limit)
    let mut peer_bandwidth = (5_000_000u32).to_be_bytes().to_vec();
    peer_bandwidth.push(0); // Hard limit type
    let set_peer_bandwidth = create_control_message(6, &peer_bandwidth);
    socket.write_all(&set_peer_bandwidth).await?;
    
    // 3. Set Chunk Size (4096 bytes)
    let chunk_size = create_control_message(1, &(4096u32).to_be_bytes());
    socket.write_all(&chunk_size).await?;

    socket.flush().await?;
    info!("✅ Initial control messages sent successfully");
    Ok(())
}

async fn send_stream_begin(socket: &mut TcpStream, stream_id: u32) -> Result<(), io::Error> {
    // User Control Message (4) - Stream Begin (0)
    let mut payload = vec![];
    payload.extend_from_slice(&0u16.to_be_bytes()); // Event type 0 = Stream Begin
    payload.extend_from_slice(&stream_id.to_be_bytes()); // Stream ID
    
    let stream_begin = create_control_message(4, &payload);
    socket.write_all(&stream_begin).await?;
    socket.flush().await?;
    Ok(())
}

fn create_control_message(message_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    
    // Chunk basic header: fmt=0 (11-byte header), chunk stream ID=2 (control stream)
    chunk.push(0x02); // fmt=0, cs_id=2
    
    // Message header (11 bytes for type 0)
    chunk.extend_from_slice(&[0, 0, 0]); // timestamp = 0
    chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes()[1..]); // message length (3 bytes)
    chunk.push(message_type); // message type ID
    chunk.extend_from_slice(&[0, 0, 0, 0]); // message stream ID = 0 (little endian)
    
    // Payload
    chunk.extend_from_slice(payload);
    
    chunk
}

fn create_command_chunk(payload: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    
    // Chunk basic header: fmt=0, chunk stream ID=3 (command/data stream)
    chunk.push(0x03); // fmt=0, cs_id=3
    
    // Message header (11 bytes for type 0)
    chunk.extend_from_slice(&[0, 0, 0]); // timestamp = 0
    chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes()[1..]); // message length (3 bytes)
    chunk.push(20); // message type ID for AMF0 command
    chunk.extend_from_slice(&[0, 0, 0, 0]); // message stream ID = 0 (little endian)
    
    // Payload
    chunk.extend_from_slice(payload);
    
    chunk
} 