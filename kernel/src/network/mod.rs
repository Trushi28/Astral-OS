// src/network/mod.rs
//! Network stack implementation

pub mod ethernet;
pub mod arp;
pub mod ip;
pub mod udp;
pub mod tcp;
pub mod socket;
pub mod device;

use alloc::vec::Vec;
use alloc::boxed::Box;
use spin::Mutex;
use device::NetworkDevice;

static NETWORK_STACK: Mutex<Option<NetworkStack>> = Mutex::new(None);

pub struct NetworkStack {
    devices: Vec<Box<dyn NetworkDevice>>,
    arp_cache: arp::ArpCache,
    socket_table: socket::SocketTable,
    ip_config: ip::IpConfig,
}

impl NetworkStack {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            arp_cache: arp::ArpCache::new(),
            socket_table: socket::SocketTable::new(),
            ip_config: ip::IpConfig::default(),
        }
    }
    
    pub fn add_device(&mut self, device: Box<dyn NetworkDevice>) {
        self.devices.push(device);
    }
    
    pub fn set_ip_config(&mut self, config: ip::IpConfig) {
        self.ip_config = config;
    }
    
    pub fn send_packet(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if let Some(device) = self.devices.first_mut() {
            device.send(packet)
        } else {
            Err("No network device available")
        }
    }
    
    pub fn poll(&mut self) {
        // Collect all packets first to avoid borrow conflict
        let mut packets = Vec::new();
        for device in &mut self.devices {
            while let Some(packet) = device.receive() {
                packets.push(packet);
            }
        }
        // Now process collected packets
        for packet in packets {
            self.process_packet(&packet);
        }
    }
    
    fn process_packet(&mut self, packet: &[u8]) {
        // Parse Ethernet frame
        if let Some(eth_frame) = ethernet::EthernetFrame::parse(packet) {
            match eth_frame.ethertype {
                ethernet::ETHERTYPE_ARP => {
                    self.handle_arp_packet(eth_frame.payload);
                }
                ethernet::ETHERTYPE_IPV4 => {
                    self.handle_ip_packet(eth_frame.payload);
                }
                _ => {
                    // Unknown ethertype, ignore
                }
            }
        }
    }
    
    fn handle_arp_packet(&mut self, payload: &[u8]) {
        if let Some(arp_packet) = arp::ArpPacket::parse(payload) {
            match arp_packet.operation {
                arp::ARP_REQUEST => {
                    // Check if request is for our IP
                    if arp_packet.target_proto_addr == self.ip_config.address.to_bytes() {
                        self.send_arp_reply(&arp_packet);
                    }
                }
                arp::ARP_REPLY => {
                    // Update ARP cache
                    let ip = ip::Ipv4Address::from_bytes(&arp_packet.sender_proto_addr);
                    let mac = ethernet::MacAddress::from_bytes(&arp_packet.sender_hw_addr);
                    self.arp_cache.insert(ip, mac);
                }
                _ => {}
            }
        }
    }
    
    fn send_arp_reply(&mut self, request: &arp::ArpPacket) {
        let reply = arp::ArpPacket::new_reply(
            &self.get_mac_address().to_bytes(),
            &self.ip_config.address.to_bytes(),
            &request.sender_hw_addr,
            &request.sender_proto_addr
        );
        
        // Bind to_bytes() result to a local variable to extend its lifetime
        let reply_bytes = reply.to_bytes();
        
        // Wrap in Ethernet frame
        let eth_frame = ethernet::EthernetFrame::new(
            ethernet::MacAddress::from_bytes(&request.sender_hw_addr),
            self.get_mac_address(),
            ethernet::ETHERTYPE_ARP,
            &reply_bytes
        );
        
        let eth_bytes = eth_frame.to_bytes();
        let _ = self.send_packet(&eth_bytes);
    }
    
    fn handle_ip_packet(&mut self, payload: &[u8]) {
        if let Some(ip_packet) = ip::Ipv4Packet::parse(payload) {
            // Check if packet is for us
            if ip_packet.dst_addr != self.ip_config.address {
                return; // Not for us
            }
            
            match ip_packet.protocol {
                ip::PROTOCOL_ICMP => {
                    self.handle_icmp_packet(&ip_packet);
                }
                ip::PROTOCOL_TCP => {
                    self.handle_tcp_packet(&ip_packet);
                }
                ip::PROTOCOL_UDP => {
                    self.handle_udp_packet(&ip_packet);
                }
                _ => {}
            }
        }
    }
    
    fn handle_icmp_packet(&mut self, ip_packet: &ip::Ipv4Packet) {
        // Simple ICMP echo reply
        if ip_packet.payload.len() >= 8 {
            let icmp_type = ip_packet.payload[0];
            
            if icmp_type == 8 { // Echo request
                self.send_icmp_reply(ip_packet);
            }
        }
    }
    
    fn send_icmp_reply(&mut self, request: &ip::Ipv4Packet) {
        let mut reply_payload = request.payload.to_vec();
        reply_payload[0] = 0; // Echo reply
        
        // Recalculate ICMP checksum
        reply_payload[2] = 0;
        reply_payload[3] = 0;
        let checksum = ip::calculate_checksum(&reply_payload);
        reply_payload[2] = (checksum >> 8) as u8;
        reply_payload[3] = checksum as u8;
        
        // Create IP packet
        let ip_reply = ip::Ipv4Packet::new(
            self.ip_config.address,
            request.src_addr,
            ip::PROTOCOL_ICMP,
            &reply_payload
        );
        
        // Look up MAC address
        if let Some(mac) = self.arp_cache.lookup(request.src_addr) {
            // Bind to_bytes() result to a local variable to extend its lifetime
            let ip_reply_bytes = ip_reply.to_bytes();
            
            let eth_frame = ethernet::EthernetFrame::new(
                mac,
                self.get_mac_address(),
                ethernet::ETHERTYPE_IPV4,
                &ip_reply_bytes
            );
            
            let eth_bytes = eth_frame.to_bytes();
            let _ = self.send_packet(&eth_bytes);
        }
    }
    
    fn handle_tcp_packet(&mut self, ip_packet: &ip::Ipv4Packet) {
        if let Some(tcp_segment) = tcp::TcpSegment::parse(ip_packet.payload) {
            self.socket_table.deliver_tcp(
                ip_packet.src_addr,
                ip_packet.dst_addr,
                tcp_segment
            );
        }
    }
    
    fn handle_udp_packet(&mut self, ip_packet: &ip::Ipv4Packet) {
        if let Some(udp_datagram) = udp::UdpDatagram::parse(ip_packet.payload) {
            self.socket_table.deliver_udp(
                ip_packet.src_addr,
                ip_packet.dst_addr,
                udp_datagram
            );
        }
    }
    
    fn get_mac_address(&self) -> ethernet::MacAddress {
        if let Some(device) = self.devices.first() {
            device.mac_address()
        } else {
            ethernet::MacAddress::new([0, 0, 0, 0, 0, 0])
        }
    }
}

pub fn init() {
    let stack = NetworkStack::new();
    *NETWORK_STACK.lock() = Some(stack);
}

pub fn add_device(device: Box<dyn NetworkDevice>) {
    let mut stack = NETWORK_STACK.lock();
    if let Some(ref mut s) = *stack {
        s.add_device(device);
    }
}

pub fn set_ip_config(address: ip::Ipv4Address, netmask: ip::Ipv4Address, gateway: ip::Ipv4Address) {
    let mut stack = NETWORK_STACK.lock();
    if let Some(ref mut s) = *stack {
        s.set_ip_config(ip::IpConfig {
            address,
            netmask,
            gateway,
        });
    }
}

pub fn poll() {
    let mut stack = NETWORK_STACK.lock();
    if let Some(ref mut s) = *stack {
        s.poll();
    }
}

pub fn send_packet(packet: &[u8]) -> Result<(), &'static str> {
    let mut stack = NETWORK_STACK.lock();
    if let Some(ref mut s) = *stack {
        s.send_packet(packet)
    } else {
        Err("Network stack not initialized")
    }
}