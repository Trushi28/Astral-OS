// src/network/device.rs

use super::ethernet::MacAddress;
use alloc::vec::Vec;

pub trait NetworkDevice: Send + Sync {
    fn mac_address(&self) -> MacAddress;
    fn send(&mut self, packet: &[u8]) -> Result<(), &'static str>;
    fn receive(&mut self) -> Option<Vec<u8>>;
    fn link_up(&self) -> bool {
        true
    }
}