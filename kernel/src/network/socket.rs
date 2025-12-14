// src/network/socket.rs

use super::ip::Ipv4Address;
use super::tcp::{TcpConnection, TcpSegment};
use super::udp::UdpDatagram;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

pub type SocketHandle = u64;

pub enum Socket {
    Tcp(TcpConnection),
    Udp(UdpSocket),
}

pub struct UdpSocket {
    pub local_addr: Ipv4Address,
    pub local_port: u16,
    pub recv_queue: Vec<(Ipv4Address, u16, Vec<u8>)>,
}

pub struct SocketTable {
    sockets: BTreeMap<SocketHandle, Socket>,
    next_handle: SocketHandle,
}

impl SocketTable {
    pub fn new() -> Self {
        Self {
            sockets: BTreeMap::new(),
            next_handle: 1,
        }
    }
    
    pub fn create_tcp(&mut self, local_addr: Ipv4Address, local_port: u16) -> SocketHandle {
        let handle = self.next_handle;
        self.next_handle += 1;
        
        let conn = TcpConnection::new(local_addr, local_port);
        self.sockets.insert(handle, Socket::Tcp(conn));
        
        handle
    }
    
    pub fn deliver_tcp(&mut self, _src: Ipv4Address, _dst: Ipv4Address, segment: TcpSegment) {
        // Find matching socket
        for socket in self.sockets.values_mut() {
            if let Socket::Tcp(ref mut conn) = socket {
                if conn.local_port == segment.dst_port {
                    conn.handle_segment(segment);
                    break;
                }
            }
        }
    }
    
    pub fn deliver_udp(&mut self, src: Ipv4Address, _dst: Ipv4Address, datagram: UdpDatagram) {
        // Find matching socket
        for socket in self.sockets.values_mut() {
            if let Socket::Udp(ref mut udp) = socket {
                if udp.local_port == datagram.dst_port {
                    udp.recv_queue.push((
                        src,
                        datagram.src_port,
                        datagram.payload.to_vec()
                    ));
                    break;
                }
            }
        }
    }
}