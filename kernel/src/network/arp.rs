// src/network/arp.rs

use super::ethernet::MacAddress;
use super::ip::Ipv4Address;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

pub const ARP_REQUEST: u16 = 1;
pub const ARP_REPLY: u16 = 2;

pub struct ArpPacket {
    pub hw_type: u16,
    pub proto_type: u16,
    pub hw_addr_len: u8,
    pub proto_addr_len: u8,
    pub operation: u16,
    pub sender_hw_addr: [u8; 6],
    pub sender_proto_addr: [u8; 4],
    pub target_hw_addr: [u8; 6],
    pub target_proto_addr: [u8; 4],
}

impl ArpPacket {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {
            return None;
        }
        
        let mut sender_hw = [0u8; 6];
        sender_hw.copy_from_slice(&data[8..14]);
        
        let mut sender_proto = [0u8; 4];
        sender_proto.copy_from_slice(&data[14..18]);
        
        let mut target_hw = [0u8; 6];
        target_hw.copy_from_slice(&data[18..24]);
        
        let mut target_proto = [0u8; 4];
        target_proto.copy_from_slice(&data[24..28]);
        
        Some(Self {
            hw_type: u16::from_be_bytes([data[0], data[1]]),
            proto_type: u16::from_be_bytes([data[2], data[3]]),
            hw_addr_len: data[4],
            proto_addr_len: data[5],
            operation: u16::from_be_bytes([data[6], data[7]]),
            sender_hw_addr: sender_hw,
            sender_proto_addr: sender_proto,
            target_hw_addr: target_hw,
            target_proto_addr: target_proto,
        })
    }
    
    pub fn new_request(sender_mac: &[u8], sender_ip: &[u8], target_ip: &[u8]) -> Self {
        let mut sender_hw = [0u8; 6];
        sender_hw.copy_from_slice(sender_mac);
        
        let mut sender_proto = [0u8; 4];
        sender_proto.copy_from_slice(sender_ip);
        
        let mut target_proto = [0u8; 4];
        target_proto.copy_from_slice(target_ip);
        
        Self {
            hw_type: 1, // Ethernet
            proto_type: 0x0800, // IPv4
            hw_addr_len: 6,
            proto_addr_len: 4,
            operation: ARP_REQUEST,
            sender_hw_addr: sender_hw,
            sender_proto_addr: sender_proto,
            target_hw_addr: [0; 6],
            target_proto_addr: target_proto,
        }
    }
    
    pub fn new_reply(sender_mac: &[u8], sender_ip: &[u8], target_mac: &[u8], target_ip: &[u8]) -> Self {
        let mut sender_hw = [0u8; 6];
        sender_hw.copy_from_slice(sender_mac);
        
        let mut sender_proto = [0u8; 4];
        sender_proto.copy_from_slice(sender_ip);
        
        let mut target_hw = [0u8; 6];
        target_hw.copy_from_slice(target_mac);
        
        let mut target_proto = [0u8; 4];
        target_proto.copy_from_slice(target_ip);
        
        Self {
            hw_type: 1,
            proto_type: 0x0800,
            hw_addr_len: 6,
            proto_addr_len: 4,
            operation: ARP_REPLY,
            sender_hw_addr: sender_hw,
            sender_proto_addr: sender_proto,
            target_hw_addr: target_hw,
            target_proto_addr: target_proto,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut packet = Vec::with_capacity(28);
        
        packet.extend_from_slice(&self.hw_type.to_be_bytes());
        packet.extend_from_slice(&self.proto_type.to_be_bytes());
        packet.push(self.hw_addr_len);
        packet.push(self.proto_addr_len);
        packet.extend_from_slice(&self.operation.to_be_bytes());
        packet.extend_from_slice(&self.sender_hw_addr);
        packet.extend_from_slice(&self.sender_proto_addr);
        packet.extend_from_slice(&self.target_hw_addr);
        packet.extend_from_slice(&self.target_proto_addr);
        
        packet
    }
}

pub struct ArpCache {
    entries: BTreeMap<u32, MacAddress>,
}

impl ArpCache {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
    
    pub fn insert(&mut self, ip: Ipv4Address, mac: MacAddress) {
        self.entries.insert(ip.to_u32(), mac);
    }
    
    pub fn lookup(&self, ip: Ipv4Address) -> Option<MacAddress> {
        self.entries.get(&ip.to_u32()).copied()
    }
    
    pub fn remove(&mut self, ip: Ipv4Address) {
        self.entries.remove(&ip.to_u32());
    }
    
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}