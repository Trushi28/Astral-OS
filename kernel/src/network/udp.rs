// src/network/udp.rs

use alloc::vec::Vec;

pub struct UdpDatagram<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub payload: &'a [u8],
}

impl<'a> UdpDatagram<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        
        let length = u16::from_be_bytes([data[4], data[5]]);
        
        if data.len() < length as usize {
            return None;
        }
        
        Some(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dst_port: u16::from_be_bytes([data[2], data[3]]),
            length,
            checksum: u16::from_be_bytes([data[6], data[7]]),
            payload: &data[8..length as usize],
        })
    }
    
    pub fn new(src_port: u16, dst_port: u16, payload: &'a [u8]) -> Self {
        Self {
            src_port,
            dst_port,
            length: (8 + payload.len()) as u16,
            checksum: 0, // Calculate later
            payload,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut datagram = Vec::with_capacity(self.length as usize);
        
        datagram.extend_from_slice(&self.src_port.to_be_bytes());
        datagram.extend_from_slice(&self.dst_port.to_be_bytes());
        datagram.extend_from_slice(&self.length.to_be_bytes());
        datagram.extend_from_slice(&self.checksum.to_be_bytes());
        datagram.extend_from_slice(self.payload);
        
        datagram
    }
}