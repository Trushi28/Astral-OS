// src/network/ethernet.rs
//! Ethernet frame handling

use alloc::vec::Vec;

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_IPV6: u16 = 0x86DD;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }
    
    pub const fn broadcast() -> Self {
        Self([0xFF; 6])
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut addr = [0u8; 6];
        addr.copy_from_slice(&bytes[..6]);
        Self(addr)
    }
    
    pub fn to_bytes(&self) -> [u8; 6] {
        self.0
    }
    
    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF; 6]
    }
    
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0x01) != 0
    }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5])
    }
}

pub struct EthernetFrame<'a> {
    pub dst_mac: MacAddress,
    pub src_mac: MacAddress,
    pub ethertype: u16,
    pub payload: &'a [u8],
}

impl<'a> EthernetFrame<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }
        
        let dst_mac = MacAddress::from_bytes(&data[0..6]);
        let src_mac = MacAddress::from_bytes(&data[6..12]);
        let ethertype = u16::from_be_bytes([data[12], data[13]]);
        let payload = &data[14..];
        
        Some(Self {
            dst_mac,
            src_mac,
            ethertype,
            payload,
        })
    }
    
    pub fn new(dst_mac: MacAddress, src_mac: MacAddress, ethertype: u16, payload: &'a [u8]) -> Self {
        Self {
            dst_mac,
            src_mac,
            ethertype,
            payload,
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut frame = Vec::with_capacity(14 + self.payload.len());
        
        frame.extend_from_slice(&self.dst_mac.0);
        frame.extend_from_slice(&self.src_mac.0);
        frame.extend_from_slice(&self.ethertype.to_be_bytes());
        frame.extend_from_slice(self.payload);
        
        // Pad to minimum frame size (64 bytes including CRC)
        while frame.len() < 60 {
            frame.push(0);
        }
        
        frame
    }
}