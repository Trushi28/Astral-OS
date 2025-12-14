// src/network/tcp.rs

use super::ip::Ipv4Address;
use alloc::collections::VecDeque;

pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

pub struct TcpSegment<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence: u32,
    pub acknowledgment: u32,
    pub data_offset: u8,
    pub flags: u8,
    pub window: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
    pub payload: &'a [u8],
}

impl<'a> TcpSegment<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }
        
        let data_offset = data[12] >> 4;
        let header_len = (data_offset * 4) as usize;
        
        if header_len > data.len() {
            return None;
        }
        
        Some(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dst_port: u16::from_be_bytes([data[2], data[3]]),
            sequence: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            acknowledgment: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            data_offset,
            flags: data[13],
            window: u16::from_be_bytes([data[14], data[15]]),
            checksum: u16::from_be_bytes([data[16], data[17]]),
            urgent_ptr: u16::from_be_bytes([data[18], data[19]]),
            payload: &data[header_len..],
        })
    }
    
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags & flag) != 0
    }
}

pub struct TcpConnection {
    pub state: TcpState,
    pub local_addr: Ipv4Address,
    pub local_port: u16,
    pub remote_addr: Ipv4Address,
    pub remote_port: u16,
    
    // Sequence numbers
    pub send_next: u32,
    pub send_unack: u32,
    pub recv_next: u32,
    
    // Buffers
    pub send_buffer: VecDeque<u8>,
    pub recv_buffer: VecDeque<u8>,
    
    // Window sizes
    pub send_window: u16,
    pub recv_window: u16,
}

impl TcpConnection {
    pub fn new(local_addr: Ipv4Address, local_port: u16) -> Self {
        Self {
            state: TcpState::Closed,
            local_addr,
            local_port,
            remote_addr: Ipv4Address::new(0, 0, 0, 0),
            remote_port: 0,
            send_next: 0,
            send_unack: 0,
            recv_next: 0,
            send_buffer: VecDeque::new(),
            recv_buffer: VecDeque::new(),
            send_window: 65535,
            recv_window: 65535,
        }
    }
    
    pub fn handle_segment(&mut self, segment: TcpSegment) {
        match self.state {
            TcpState::Listen => {
                if segment.has_flag(TCP_FLAG_SYN) {
                    self.recv_next = segment.sequence.wrapping_add(1);
                    self.state = TcpState::SynReceived;
                    // Send SYN-ACK
                }
            }
            TcpState::Established => {
                if segment.has_flag(TCP_FLAG_ACK) {
                    // Update send window
                    self.send_unack = segment.acknowledgment;
                }
                
                if !segment.payload.is_empty() {
                    // Add data to receive buffer
                    self.recv_buffer.extend(segment.payload);
                    self.recv_next = self.recv_next.wrapping_add(segment.payload.len() as u32);
                    // Send ACK
                }
                
                if segment.has_flag(TCP_FLAG_FIN) {
                    self.state = TcpState::CloseWait;
                    // Send ACK
                }
            }
            _ => {}
        }
    }
}