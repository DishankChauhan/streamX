use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct RtmpMessage {
    pub message_type: MessageType,
    pub payload: Bytes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Connect,
    Publish,
    Audio,
    Video,
    SetChunkSize,
    Abort,
    Acknowledgement,
    WindowAcknowledgementSize,
    SetPeerBandwidth,
    Command,
    Unknown,
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        match value {
            1 => MessageType::SetChunkSize,
            2 => MessageType::Abort,
            3 => MessageType::Acknowledgement,
            5 => MessageType::WindowAcknowledgementSize,
            6 => MessageType::SetPeerBandwidth,
            8 => MessageType::Audio,
            9 => MessageType::Video,
            20 => MessageType::Command, // AMF0 Command
            17 => MessageType::Command, // AMF3 Command
            _ => MessageType::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct RtmpHeader {
    pub format: u8,
    pub chunk_stream_id: u32,
    pub timestamp: u32,
    pub message_length: u32,
    pub message_type_id: u8,
    pub message_stream_id: u32,
}

#[derive(Debug)]
pub struct ConnectCommand {
    pub app: String,
    pub flash_ver: String,
    pub tc_url: String,
}

#[derive(Debug)]
pub struct PublishCommand {
    pub stream_key: String,
    pub publish_type: String,
}

#[derive(Debug)]
pub struct CreateStreamCommand {
    pub transaction_id: f64,
}

impl RtmpHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.is_empty() {
            return None;
        }

        let first_byte = data[0];
        let format = (first_byte >> 6) & 0x03;
        let mut chunk_stream_id = (first_byte & 0x3f) as u32;
        let mut offset = 1;

        // Handle extended chunk stream IDs
        if chunk_stream_id == 0 {
            if data.len() < 2 {
                return None;
            }
            chunk_stream_id = data[1] as u32 + 64;
            offset = 2;
        } else if chunk_stream_id == 1 {
            if data.len() < 3 {
                return None;
            }
            chunk_stream_id = ((data[2] as u32) << 8) + data[1] as u32 + 64;
            offset = 3;
        }

        let header_size = match format {
            0 => 11, // Type 0: Full header
            1 => 7,  // Type 1: No message stream ID
            2 => 3,  // Type 2: Timestamp delta only
            3 => 0,  // Type 3: No header fields
            _ => return None,
        };

        if data.len() < offset + header_size {
            return None;
        }

        let (timestamp, message_length, message_type_id, message_stream_id) = match format {
            0 => {
                let timestamp = u32::from_be_bytes([0, data[offset], data[offset + 1], data[offset + 2]]);
                let message_length = u32::from_be_bytes([0, data[offset + 3], data[offset + 4], data[offset + 5]]);
                let message_type_id = data[offset + 6];
                let message_stream_id = u32::from_le_bytes([
                    data[offset + 7], data[offset + 8], data[offset + 9], data[offset + 10]
                ]);
                (timestamp, message_length, message_type_id, message_stream_id)
            }
            1 => {
                let timestamp = u32::from_be_bytes([0, data[offset], data[offset + 1], data[offset + 2]]);
                let message_length = u32::from_be_bytes([0, data[offset + 3], data[offset + 4], data[offset + 5]]);
                let message_type_id = data[offset + 6];
                (timestamp, message_length, message_type_id, 0)
            }
            2 => {
                let timestamp = u32::from_be_bytes([0, data[offset], data[offset + 1], data[offset + 2]]);
                (timestamp, 0, 0, 0)
            }
            3 => (0, 0, 0, 0),
            _ => unreachable!(),
        };

        Some((
            RtmpHeader {
                format,
                chunk_stream_id,
                timestamp,
                message_length,
                message_type_id,
                message_stream_id,
            },
            offset + header_size,
        ))
    }
}

pub fn parse_rtmp_connect(payload: &[u8]) -> Option<ConnectCommand> {
    // Simple AMF0 parsing for connect command
    // This is a minimal implementation for connect command
    
    if payload.len() < 10 {
        return None;
    }

    // Skip command name "connect" (AMF0 string)
    let mut offset = 0;
    
    // AMF0 String marker (0x02)
    if payload[offset] != 0x02 {
        return None;
    }
    offset += 1;
    
    // String length
    let str_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2;
    
    if offset + str_len > payload.len() {
        return None;
    }
    
    let command_name = String::from_utf8_lossy(&payload[offset..offset + str_len]);
    offset += str_len;
    
    if command_name != "connect" {
        return None;
    }
    
    // Skip transaction ID (AMF0 Number - 0x00 + 8 bytes)
    if payload.len() < offset + 9 {
        return None;
    }
    offset += 9;
    
    // Parse command object (AMF0 Object - 0x03)
    if payload.len() < offset + 1 || payload[offset] != 0x03 {
        return None;
    }
    offset += 1;
    
    let mut app = String::new();
    let mut flash_ver = String::new();
    let mut tc_url = String::new();
    
    // Parse object properties
    while offset < payload.len() {
        // Check for object end marker (0x00 0x00 0x09)
        if offset + 3 <= payload.len() && 
           payload[offset] == 0x00 && payload[offset + 1] == 0x00 && payload[offset + 2] == 0x09 {
            break;
        }
        
        // Property name length
        if offset + 2 > payload.len() {
            break;
        }
        let prop_name_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
        offset += 2;
        
        if offset + prop_name_len > payload.len() {
            break;
        }
        
        let prop_name = String::from_utf8_lossy(&payload[offset..offset + prop_name_len]);
        offset += prop_name_len;
        
        // Property value type
        if offset >= payload.len() {
            break;
        }
        
        let value_type = payload[offset];
        offset += 1;
        
        match value_type {
            0x02 => { // String
                if offset + 2 > payload.len() {
                    break;
                }
                let value_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
                offset += 2;
                
                if offset + value_len > payload.len() {
                    break;
                }
                
                let value = String::from_utf8_lossy(&payload[offset..offset + value_len]).to_string();
                offset += value_len;
                
                match prop_name.as_ref() {
                    "app" => app = value,
                    "flashVer" => flash_ver = value,
                    "tcUrl" => tc_url = value,
                    _ => {}
                }
            }
            0x00 => { // Number - skip 8 bytes
                offset += 8;
            }
            0x01 => { // Boolean - skip 1 byte
                offset += 1;
            }
            _ => {
                // Unknown type, skip
                break;
            }
        }
    }
    
    Some(ConnectCommand { app, flash_ver, tc_url })
}

pub fn parse_rtmp_publish(payload: &[u8]) -> Option<PublishCommand> {
    if payload.len() < 10 {
        return None;
    }

    let mut offset = 0;
    
    // AMF0 String marker (0x02)
    if payload[offset] != 0x02 {
        return None;
    }
    offset += 1;
    
    // String length
    let str_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2;
    
    if offset + str_len > payload.len() {
        return None;
    }
    
    let command_name = String::from_utf8_lossy(&payload[offset..offset + str_len]);
    offset += str_len;
    
    if command_name != "publish" {
        return None;
    }
    
    // Skip transaction ID (AMF0 Number - 0x00 + 8 bytes)
    if payload.len() < offset + 9 {
        return None;
    }
    offset += 9;
    
    // Skip null (AMF0 Null - 0x05)
    if payload.len() < offset + 1 || payload[offset] != 0x05 {
        return None;
    }
    offset += 1;
    
    // Parse stream key (AMF0 String)
    if payload.len() < offset + 3 || payload[offset] != 0x02 {
        return None;
    }
    offset += 1;
    
    let stream_key_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2;
    
    if offset + stream_key_len > payload.len() {
        return None;
    }
    
    let stream_key = String::from_utf8_lossy(&payload[offset..offset + stream_key_len]).to_string();
    offset += stream_key_len;
    
    // Parse publish type (AMF0 String) - optional
    let mut publish_type = String::from("live");
    if offset + 3 <= payload.len() && payload[offset] == 0x02 {
        offset += 1;
        let type_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
        offset += 2;
        
        if offset + type_len <= payload.len() {
            publish_type = String::from_utf8_lossy(&payload[offset..offset + type_len]).to_string();
        }
    }
    
    Some(PublishCommand { stream_key, publish_type })
}

pub fn parse_rtmp_createstream(payload: &[u8]) -> Option<CreateStreamCommand> {
    if payload.len() < 10 {
        return None;
    }

    let mut offset = 0;
    
    // AMF0 String marker (0x02)
    if payload[offset] != 0x02 {
        return None;
    }
    offset += 1;
    
    // String length
    let str_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2;
    
    if offset + str_len > payload.len() {
        return None;
    }
    
    let command_name = String::from_utf8_lossy(&payload[offset..offset + str_len]);
    offset += str_len;
    
    if command_name != "createStream" {
        return None;
    }
    
    // Parse transaction ID (AMF0 Number - 0x00 + 8 bytes)
    if payload.len() < offset + 9 {
        return None;
    }
    
    if payload[offset] != 0x00 {
        return None;
    }
    offset += 1;
    
    let transaction_id = f64::from_be_bytes([
        payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3],
        payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]
    ]);
    
    Some(CreateStreamCommand { transaction_id })
}

pub fn create_connect_response() -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "_result" (AMF0 String)
    response.push(0x02); // String marker
    response.extend_from_slice(&7u16.to_be_bytes()); // Length
    response.extend_from_slice(b"_result");
    
    // Transaction ID (1.0) (AMF0 Number)
    response.push(0x00); // Number marker
    response.extend_from_slice(&1.0f64.to_be_bytes());
    
    // Properties object (AMF0 Object)
    response.push(0x03); // Object marker
    
    // fmsVer property
    response.extend_from_slice(&6u16.to_be_bytes());
    response.extend_from_slice(b"fmsVer");
    response.push(0x02); // String marker
    response.extend_from_slice(&9u16.to_be_bytes());
    response.extend_from_slice(b"FMS/3,0,1");
    
    // capabilities property
    response.extend_from_slice(&12u16.to_be_bytes());
    response.extend_from_slice(b"capabilities");
    response.push(0x00); // Number marker
    response.extend_from_slice(&31.0f64.to_be_bytes());
    
    // Object end marker
    response.extend_from_slice(&[0x00, 0x00, 0x09]);
    
    // Information object (AMF0 Object)
    response.push(0x03); // Object marker
    
    // level property
    response.extend_from_slice(&5u16.to_be_bytes());
    response.extend_from_slice(b"level");
    response.push(0x02); // String marker
    response.extend_from_slice(&6u16.to_be_bytes());
    response.extend_from_slice(b"status");
    
    // code property
    response.extend_from_slice(&4u16.to_be_bytes());
    response.extend_from_slice(b"code");
    response.push(0x02); // String marker
    response.extend_from_slice(&29u16.to_be_bytes());
    response.extend_from_slice(b"NetConnection.Connect.Success");
    
    // description property
    response.extend_from_slice(&11u16.to_be_bytes());
    response.extend_from_slice(b"description");
    response.push(0x02); // String marker
    response.extend_from_slice(&15u16.to_be_bytes());
    response.extend_from_slice(b"Connection succeeded");
    
    // Object end marker
    response.extend_from_slice(&[0x00, 0x00, 0x09]);
    
    response
}

pub fn create_publish_response(stream_key: &str) -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "onStatus"
    response.push(0x02); // String marker
    response.extend_from_slice(&8u16.to_be_bytes()); // Length
    response.extend_from_slice(b"onStatus");
    
    // Transaction ID (0.0)
    response.push(0x00); // Number marker
    response.extend_from_slice(&0.0f64.to_be_bytes());
    
    // Null
    response.push(0x05);
    
    // Information object
    response.push(0x03); // Object marker
    
    // level property
    response.extend_from_slice(&5u16.to_be_bytes());
    response.extend_from_slice(b"level");
    response.push(0x02); // String value
    response.extend_from_slice(&6u16.to_be_bytes());
    response.extend_from_slice(b"status");
    
    // code property
    response.extend_from_slice(&4u16.to_be_bytes());
    response.extend_from_slice(b"code");
    response.push(0x02); // String value
    response.extend_from_slice(&26u16.to_be_bytes());
    response.extend_from_slice(b"NetStream.Publish.Start");
    
    // description property
    response.extend_from_slice(&11u16.to_be_bytes());
    response.extend_from_slice(b"description");
    response.push(0x02); // String value
    let desc = format!("Started publishing stream {}", stream_key);
    response.extend_from_slice(&(desc.len() as u16).to_be_bytes());
    response.extend_from_slice(desc.as_bytes());
    
    // Object end
    response.extend_from_slice(&[0x00, 0x00, 0x09]);
    
    response
}

pub fn create_createstream_response(transaction_id: f64) -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "_result"
    response.push(0x02); // String marker
    response.extend_from_slice(&7u16.to_be_bytes()); // Length
    response.extend_from_slice(b"_result");
    
    // Transaction ID
    response.push(0x00); // Number marker
    response.extend_from_slice(&transaction_id.to_be_bytes());
    
    // Null
    response.push(0x05);
    
    // Stream ID (1.0)
    response.push(0x00); // Number marker
    response.extend_from_slice(&1.0f64.to_be_bytes());
    
    response
}

pub fn parse_command_name(payload: &[u8]) -> Option<String> {
    if payload.len() < 5 {
        return None;
    }

    // AMF0 String marker (0x02)
    if payload[0] != 0x02 {
        return None;
    }
    
    // String length
    let str_len = u16::from_be_bytes([payload[1], payload[2]]) as usize;
    
    if payload.len() < 3 + str_len {
        return None;
    }
    
    let command_name = String::from_utf8_lossy(&payload[3..3 + str_len]).to_string();
    Some(command_name)
}

pub fn parse_checkbw_command(payload: &[u8]) -> Option<f64> {
    if payload.len() < 20 {
        return None;
    }

    let mut offset = 0;
    
    // Skip command name "_checkbw" (AMF0 string)
    if payload[offset] != 0x02 {
        return None;
    }
    offset += 1;
    
    let str_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
    offset += 2;
    
    if offset + str_len > payload.len() || str_len != 8 {
        return None;
    }
    
    if &payload[offset..offset + 8] != b"_checkbw" {
        return None;
    }
    offset += 8;
    
    // Parse transaction ID (AMF0 Number)
    if offset + 9 > payload.len() || payload[offset] != 0x00 {
        return None;
    }
    offset += 1;
    
    let transaction_id = f64::from_be_bytes([
        payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3],
        payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]
    ]);
    
    Some(transaction_id)
}

pub fn create_generic_response(command: &str) -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "_result"
    response.push(0x02); // String marker
    response.extend_from_slice(&7u16.to_be_bytes()); // Length
    response.extend_from_slice(b"_result");
    
    // Transaction ID (2.0)
    response.push(0x00); // Number marker
    response.extend_from_slice(&2.0f64.to_be_bytes());
    
    // Null
    response.push(0x05);
    
    // Null
    response.push(0x05);
    
    response
}

pub fn create_onbwdone_message() -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "onBWDone" (AMF0 String)
    response.push(0x02); // String marker
    response.extend_from_slice(&8u16.to_be_bytes()); // Length
    response.extend_from_slice(b"onBWDone");
    
    // Transaction ID (0.0) (AMF0 Number)
    response.push(0x00); // Number marker
    response.extend_from_slice(&0.0f64.to_be_bytes());
    
    // Null (AMF0 Null)
    response.push(0x05);
    
    response
}

pub fn create_checkbw_response(transaction_id: f64) -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "_result" (AMF0 String)
    response.push(0x02); // String marker
    response.extend_from_slice(&7u16.to_be_bytes()); // Length
    response.extend_from_slice(b"_result");
    
    // Transaction ID (AMF0 Number)
    response.push(0x00); // Number marker
    response.extend_from_slice(&transaction_id.to_be_bytes());
    
    // Null (AMF0 Null)
    response.push(0x05);
    
    // Bandwidth value (AMF0 Number) - fake bandwidth result
    response.push(0x00); // Number marker
    response.extend_from_slice(&1000000.0f64.to_be_bytes()); // 1Mbps
    
    response
}

pub fn create_onbwcheck_message() -> Vec<u8> {
    let mut response = Vec::new();
    
    // Command name "onBWCheck" (AMF0 String)
    response.push(0x02); // String marker
    response.extend_from_slice(&9u16.to_be_bytes()); // Length
    response.extend_from_slice(b"onBWCheck");
    
    // Transaction ID (0.0) (AMF0 Number)
    response.push(0x00); // Number marker
    response.extend_from_slice(&0.0f64.to_be_bytes());
    
    // Null (AMF0 Null)
    response.push(0x05);
    
    // Bandwidth value (AMF0 Number)
    response.push(0x00); // Number marker
    response.extend_from_slice(&1000000.0f64.to_be_bytes()); // 1Mbps
    
    response
} 