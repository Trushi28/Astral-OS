// src/network/ip.rs

use alloc::vec::Vec;

pub const PROTOCOL_ICMP: u8 = 1;
pub const PROTOCOL_TCP: u8 = 6;
pub const PROTOCOL_UDP: u8 = 17;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ipv4Address([u8; 4]);

impl Ipv4Address {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut addr = [0u8; 4];
        addr.copy_from_slice(&bytes[..4]);
        Self(addr)
    }
    
    pub fn to_bytes(&self) -> [u8; 4] {
        self.0
    }
    
    pub fn to_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }
    
    pub fn from_u32(val: u32) -> Self {
        Self(val.to_be_bytes())
    }
}

impl core::fmt::Display for Ipv4Address {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

#[derive(Clone, Debug)]
pub struct IpConfig {
    pub address: Ipv4Address,
    pub netmask: Ipv4Address,
    pub gateway: Ipv4Address,
}

impl Default for IpConfig {
    fn default() -> Self {
        Self {
            address: Ipv4Address::new(10, 0, 2, 15),
            netmask: Ipv4Address::new(255, 255, 255, 0),
            gateway: Ipv4Address::new(10, 0, 2, 2),
        }
    }
}

pub struct Ipv4Packet<'a> {
    pub version: u8,
    pub ihl: u8,
    pub dscp: u8,
    pub ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags: u8,
    pub fragment_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_addr: Ipv4Address,
    pub dst_addr: Ipv4Address,
    pub payload: &'a [u8],
}

impl<'a> Ipv4Packet<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }
        
        let version = data[0] >> 4;
        let ihl = data[0] & 0x0F;
        let header_len = (ihl * 4) as usize;
        
        if version != 4 || header_len > data.len() {
            return None;
        }
        
        let total_length = u16::from_be_bytes([data[2], data[3]]);
        let src_addr = Ipv4Address::from_bytes(&data[12..16]);
        let dst_addr = Ipv4Address::from_bytes(&data[16..20]);
        
        Some(Self {
            version,
            ihl,
            dscp: data[1] >> 2,
            ecn: data[1] & 0x03,
            total_length,
            identification: u16::from_be_bytes([data[4], data[5]]),
            flags: data[6] >> 5,
            fragment_offset: u16::from_be_bytes([data[6] & 0x1F, data[7]]),
            ttl: data[8],
            protocol: data[9],
            checksum: u16::from_be_bytes([data[10], data[11]]),
            src_addr,
            dst_addr,
            payload: &data[header_len..],
        })
    }
    
    pub fn new(src: Ipv4Address, dst: Ipv4Address, protocol: u8, payload: &'a [u8]) -> Self {
        Self {
            version: 4,
            ihl: 5,
            dscp: 0,
            ecn: 0,
            total_length: (20 + payload.len()) as u16,
            identification: 0,
            flags: 0,
            fragment_offset: 0,
            ttl: 64,
            protocol,
            checksum: 0,
            src_addr: src,
            dst_addr: dst,
            payload,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut packet = Vec::with_capacity(20 + self.payload.len());
        
        packet.push((self.version << 4) | self.ihl);
        packet.push((self.dscp << 2) | self.ecn);
        packet.extend_from_slice(&self.total_length.to_be_bytes());
        packet.extend_from_slice(&self.identification.to_be_bytes());
        
        let flags_frag = ((self.flags as u16) << 13) | self.fragment_offset;
        packet.extend_from_slice(&flags_frag.to_be_bytes());
        
        packet.push(self.ttl);
        packet.push(self.protocol);
        packet.extend_from_slice(&[0, 0]); // Checksum placeholder
        packet.extend_from_slice(&self.src_addr.0);
        packet.extend_from_slice(&self.dst_addr.0);
        
        // Calculate checksum
        let checksum = calculate_checksum(&packet[..20]);
        packet[10] = (checksum >> 8) as u8;
        packet[11] = checksum as u8;
        
        packet.extend_from_slice(self.payload);
        
        packet
    }
}

pub fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    
    for i in (0..data.len()).step_by(2) {
        let word = if i + 1 < data.len() {
            u16::from_be_bytes([data[i], data[i + 1]]) as u32
        } else {
            (data[i] as u32) << 8
        };
        
        sum += word;
        
        if sum > 0xFFFF {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
    }
    
    !sum as u16
}