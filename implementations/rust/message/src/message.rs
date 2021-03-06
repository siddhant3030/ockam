#![allow(unused)]

// Definition and implementation of an Ockam message and message components.
// Each message component, and the message overall, implements the "Codec" trait
// allowing it to be encoded/decoded for transmission over a transport.

pub mod message {
    use std::convert::{Into, TryFrom};
    use std::error::Error;
    use std::fmt::Formatter;
    pub use std::io::{ErrorKind, Read, Write};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::ops::Add;
    use std::slice;
    use std::sync::atomic::Ordering::AcqRel;

    const WIRE_PROTOCOL_VERSION: u8 = 1;

    pub trait Codec {
        type Inner;

        fn encode(t: &Self::Inner, v: &mut Vec<u8>) -> Result<(), String>;
        fn decode(s: &[u8]) -> Result<(Self::Inner, &[u8]), String>;
        fn decode_boxed(s: &[u8]) -> Result<(Box<Self::Inner>, &[u8]), String> {
            Err("not implemented".to_string())
        }
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct Message {
        pub onward_route: Route,
        pub return_route: Route,
        pub message_body: Vec<u8>,
    }

    impl Default for Message {
        fn default() -> Message {
            Message {
                onward_route: Route { addresses: vec![] },
                return_route: Route { addresses: vec![] },
                message_body: vec![0],
            }
        }
    }

    impl Codec for Message {
        type Inner = Message;
        fn encode(msg: &Message, u: &mut Vec<u8>) -> Result<(), String> {
            Route::encode(&msg.onward_route, u);
            Route::encode(&msg.return_route, u);
            u.extend(&msg.message_body[0..]);
            Ok(())
        }

        fn decode(u: &[u8]) -> Result<(Message, &[u8]), String> {
            let mut msg = Message::default();
            let mut w = u;
            match Route::decode(w) {
                Ok((r, u1)) => {
                    msg.onward_route = r;
                    w = u1;
                }
                Err(s) => {
                    return Err(s);
                }
            }
            match Route::decode(w) {
                Ok((r, u1)) => {
                    msg.return_route = r;
                    w = u1;
                }
                Err(s) => {
                    return Err(s);
                }
            }
            msg.message_body.append(&mut (w.to_vec()));
            Ok((msg, w))
        }
        fn decode_boxed(u: &[u8]) -> Result<(Box<Message>, &[u8]), String> {
            let mut msg = Box::new(Message::default());
            let mut w = u;
            match Route::decode(w) {
                Ok((r, u1)) => {
                    msg.onward_route = r;
                    w = u1;
                }
                Err(s) => {
                    return Err(s);
                }
            }
            match Route::decode(w) {
                Ok((r, u1)) => {
                    msg.return_route = r;
                    w = u1;
                }
                Err(s) => {
                    return Err(s);
                }
            }
            msg.message_body.append(&mut (w.to_vec()));
            Ok((msg, w))
        }
    }

    /* Addresses */
    #[repr(C)]
    pub enum AddressType {
        Local = 0,
        Tcp = 1,
        Udp = 2,
    }

    impl Clone for AddressType {
        fn clone(&self) -> Self {
            return match self {
                AddressType::Local => AddressType::Local,
                AddressType::Tcp => AddressType::Tcp,
                AddressType::Udp => AddressType::Udp,
            };
        }
    }

    impl std::fmt::Debug for AddressType {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
            let s: String;
            match self {
                AddressType::Local => {
                    s = "Local".to_string();
                }
                AddressType::Tcp => {
                    s = "Tcp".to_string();
                }
                AddressType::Udp => {
                    s = "Udp".to_string();
                }
            }
            f.debug_struct("AddressType").field("Type", &s).finish();
            Ok(())
        }
    }

    impl Copy for AddressType {}

    #[derive(Debug, PartialEq)]
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct LocalAddress {
        pub address: u32,
    }

    // ToDo: implement Copy, Clone
    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Address {
        LocalAddress(AddressType, LocalAddress),
        TcpAddress(AddressType, IpAddr, u16),
        UdpAddress(AddressType, IpAddr, u16),
    }

    pub enum HostAddressType {
        Ipv4 = 0,
        Ipv6 = 1,
    }

    impl PartialEq for AddressType {
        fn eq(&self, other: &Self) -> bool {
            let t: u8 = *self as u8;
            let o = *other as u8;
            return t == o;
        }

        fn ne(&self, other: &Self) -> bool {
            let t = *self as u8;
            let o = *other as u8;
            return t != o;
        }
    }

    impl TryFrom<u8> for HostAddressType {
        type Error = String;
        fn try_from(data: u8) -> Result<Self, Self::Error> {
            match data {
                0 => Ok(HostAddressType::Ipv4),
                1 => Ok(HostAddressType::Ipv6),
                _ => Err("Unknown host address type".to_string()),
            }
        }
    }

    impl TryFrom<u8> for AddressType {
        type Error = String;
        fn try_from(data: u8) -> Result<Self, Self::Error> {
            match data {
                0 => Ok(AddressType::Local),
                1 => Ok(AddressType::Tcp),
                2 => Ok(AddressType::Udp),
                _ => Err("Unknown address type".to_string()),
            }
        }
    }

    impl Codec for Address {
        type Inner = Address;
        fn encode(a: &Address, v: &mut Vec<u8>) -> Result<(), String> {
            match a {
                Address::LocalAddress(mut t, a) => {
                    v.push(t as u8);
                    LocalAddress::encode(a, v);
                }
                Address::UdpAddress(mut t, ipa, mut port) => {
                    v.push(t as u8);
                    IpAddr::encode(ipa, v);
                    v.append(&mut port.to_le_bytes().to_vec());
                }
                Address::TcpAddress(mut t, ipa, mut port) => {
                    v.push(t as u8);
                    IpAddr::encode(ipa, v);
                    v.append(&mut port.to_le_bytes().to_vec());
                }
            }
            Ok(())
        }
        fn decode(u: &[u8]) -> Result<(Address, &[u8]), String> {
            match AddressType::try_from(u[0])? {
                AddressType::Local => {
                    let (la, v) = LocalAddress::decode(&u[1..])?;
                    let address = Address::LocalAddress(AddressType::Local, la);
                    Ok((address, v))
                }
                AddressType::Tcp => Err("Not Implemented".to_string()),
                AddressType::Udp => {
                    let (ipa, v) = IpAddr::decode(&u[1..])?;
                    let port = u16::from_le_bytes([v[0], v[1]]);
                    let address = Address::UdpAddress(AddressType::Udp, ipa, port);
                    Ok((address, &v[2..]))
                }
            }
        }
    }

    impl Codec for IpAddr {
        type Inner = IpAddr;
        fn encode(ip: &IpAddr, v: &mut Vec<u8>) -> Result<(), String> {
            match ip {
                std::net::IpAddr::V4(ip4) => {
                    v.push(HostAddressType::Ipv4 as u8);
                    v.extend_from_slice(ip4.octets().as_ref());
                }
                std::net::IpAddr::V6(ip6) => {
                    v.push(HostAddressType::Ipv6 as u8);
                    v.extend_from_slice(ip6.octets().as_ref());
                }
            }
            Ok(())
        }
        fn decode(u: &[u8]) -> Result<(IpAddr, &[u8]), String> {
            match (HostAddressType::try_from(u[0])?, &u[1..]) {
                (HostAddressType::Ipv4, addr) => {
                    let ip4 = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
                    let ip_addr = IpAddr::V4(ip4);
                    Ok((ip_addr, &u[5..]))
                }
                _ => Err("".to_string()),
            }
        }
    }

    impl Codec for LocalAddress {
        type Inner = LocalAddress;
        fn encode(la: &LocalAddress, u: &mut Vec<u8>) -> Result<(), String> {
            for le_byte in la.address.to_le_bytes().iter() {
                u.push(*le_byte);
            }
            Ok(())
        }
        fn decode(u: &[u8]) -> Result<(LocalAddress, &[u8]), String> {
            Ok((
                LocalAddress {
                    address: u32::from_le_bytes([u[0], u[1], u[2], u[3]]),
                },
                &u[4..],
            ))
        }
    }

    /* Routes */
    #[derive(Debug)]
    #[repr(C)]
    pub struct Route {
        pub addresses: Vec<Address>,
    }

    impl Clone for Route {
        fn clone(&self) -> Self {
            return Route {
                addresses: self.addresses.clone(),
            };
        }

        fn clone_from(&mut self, source: &Self) {
            unimplemented!()
        }
    }

    impl Codec for Route {
        type Inner = Route;
        fn encode(route: &Route, u: &mut Vec<u8>) -> Result<(), String> {
            if route.addresses.is_empty() {
                u.push(0 as u8)
            } else {
                u.push(route.addresses.len() as u8);
                for i in 0..route.addresses.len() {
                    Address::encode(&route.addresses[i], u);
                }
            }
            Ok(())
        }
        fn decode(encoded: &[u8]) -> Result<(Route, &[u8]), String> {
            let mut route = Route { addresses: vec![] };
            let mut next_address = &encoded[1..];
            if 0 < encoded[0] {
                for i in 0..encoded[0] as usize {
                    match Address::decode(next_address) {
                        Ok((a, x)) => {
                            route.addresses.push(a);
                            next_address = x;
                        }
                        Err(s) => {}
                    }
                }
            }
            Ok((route, next_address))
        }
    }

    // ToDo: Implement PartialEq, Eq, Copy, Clone

    // u16's are encoded as variable-length.
    // - If the value is < 0x80, it is encoded as-is, in one byte
    // - If the value is <= 0x80, the highest-order of the low-order byte is moved to the
    //   lowest-order bit in the high-order byte, and the high-order byte is shifted left by one to
    //   make room.
    impl Codec for u16 {
        type Inner = u16;
        fn encode(ul2: &u16, u: &mut Vec<u8>) -> Result<(), String> {
            if ul2 >= &mut 0xC000 {
                return Err("Maximum value exceeded".to_string());
            }
            let mut bytes = ul2.to_le_bytes();

            if ul2 < &mut 0x80 {
                u.push(bytes[0])
            } else {
                bytes[1] <<= 0x01;
                if 0 != (bytes[0] & 0x80) {
                    bytes[1] |= 0x01;
                }
                bytes[0] |= 0x80;
                u.push(bytes[0]);
                u.push(bytes[1])
            }
            Ok(())
        }

        fn decode(u: &[u8]) -> Result<(u16, &[u8]), String> {
            let mut bytes = [0, 0];
            let mut i = 1;

            bytes[0] = u[0] & 0x7f;
            if (u[0] & 0x80) == 0x80 as u8 {
                bytes[0] += (u[1] & 0x01) << 7;
                bytes[1] = u[1] >> 1;
                i = 2;
            }
            let ul2 = ((bytes[1] as u16) << 8) + bytes[0] as u16;

            Ok((ul2, &u[i..]))
        }
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct WireProtocolVersion {
        pub v: u16,
    }

    impl Default for WireProtocolVersion {
        fn default() -> WireProtocolVersion {
            WireProtocolVersion { v: 1 }
        }
    }

    // std::io::Read & std::io::Write trait implementation
    impl std::io::Read for Message {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
            if buf.len() < self.message_body.len() {
                return Err(std::io::Error::new(ErrorKind::Other, "buffer too small"));
            }
            for i in 0..self.message_body.len() {
                buf[i] = self.message_body[i];
            }
            Ok(self.message_body.len())
        }
    }
    impl std::io::Write for Message {
        fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
            if !self.message_body.is_empty() {
                return Err(std::io::Error::new(ErrorKind::Other, "no message body"));
            }
            for i in 0..buf.len() {
                self.message_body.push(buf[i]);
            }
            Ok(self.message_body.len())
        }
        fn flush(&mut self) -> Result<(), std::io::Error> {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn local_address_codec() {
        let mut local_in = LocalAddress {
            address: 0x00010203,
        };
        let mut v: Vec<u8> = vec![];
        LocalAddress::encode(&mut local_in, &mut v);
        assert_eq!(v, [3, 2, 1, 0]);
        match LocalAddress::decode(&v) {
            Ok((local_out, w)) => assert_eq!(
                local_out,
                LocalAddress {
                    address: 0x00010203
                }
            ),
            Err(s) => {
                println!("{:?}", s);
            }
        }
    }

    #[test]
    fn ip4_address_codec() {
        let mut v: Vec<u8> = vec![];
        let mut ip4a: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        IpAddr::encode(&mut ip4a, &mut v);
        assert_eq!(v, vec![0, 127, 0, 0, 1]);
        let mut v: Vec<u8> = vec![0, 127, 0, 0, 1];
        match IpAddr::decode(&v) {
            Ok((ip4a, w)) => {
                assert_eq!(ip4a, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
            }
            Err(s) => {
                println!("{}", s);
            }
        }
    }

    #[test]
    fn address_codec() {
        let mut address = Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            0x8080,
        );
        let mut v: Vec<u8> = vec![];
        Address::encode(&mut address, &mut v);
        assert_eq!(v, vec![2, 0, 127, 0, 0, 1, 0x80, 0x80]);
        let mut v = vec![2, 0, 127, 0, 0, 1, 0x80, 0x80];
        match Address::decode(&mut v) {
            Ok((address, w)) => {
                assert_eq!(
                    address,
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        0x8080
                    )
                );
            }
            Err(s) => {
                println!("{}", s);
            }
        }
        let mut address = Address::LocalAddress(
            AddressType::Local,
            LocalAddress {
                address: 0x00010203,
            },
        );
        let mut v: Vec<u8> = vec![];
        Address::encode(&mut address, &mut v);
        assert_eq!(v, vec![0, 3, 2, 1, 0]);
        let mut v = vec![0, 3, 2, 1, 0];
        match Address::decode(&mut v) {
            Ok((address, w)) => {
                assert_eq!(
                    address,
                    Address::LocalAddress(
                        AddressType::Local,
                        LocalAddress {
                            address: 0x00010203
                        }
                    )
                );
            }
            Err(s) => {
                println!("{}", s);
            }
        }
    }

    #[test]
    fn route_codec() {
        let mut route: Route = Route { addresses: vec![] };
        route.addresses.push(Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            0x8080,
        ));
        route.addresses.push(Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 10)),
            0x7070,
        ));
        route.addresses.push(Address::LocalAddress(
            AddressType::Local,
            LocalAddress {
                address: 0x00010203,
            },
        ));
        let mut v: Vec<u8> = vec![];
        Route::encode(&mut route, &mut v);
        assert_eq!(
            v,
            vec![3, 2, 0, 127, 0, 0, 1, 0x80, 0x80, 2, 0, 10, 0, 1, 10, 0x70, 0x70, 0, 3, 2, 1, 0]
        );
        match Route::decode(&v) {
            Ok((r, u)) => {
                assert_eq!(r.addresses.len(), 3);
                assert_eq!(
                    r.addresses[0],
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        0x8080
                    )
                );
                assert_eq!(
                    r.addresses[1],
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(10, 0, 1, 10)),
                        0x7070
                    )
                );
                assert_eq!(
                    r.addresses[2],
                    Address::LocalAddress(
                        AddressType::Local,
                        LocalAddress {
                            address: 0x00010203
                        }
                    )
                );
                assert_eq!(v.len(), 22);
            }
            Err(s) => {
                panic!();
            }
        }
    }

    #[test]
    fn u16_codec() {
        let mut u: Vec<u8> = vec![];
        let mut n: u16 = 0x7f;
        u16::encode(&mut n, &mut u);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0], 0x7f);
        match u16::decode(&u) {
            Ok((m, v)) => {
                assert_eq!(u[0], 0x7f);
                assert_eq!(v.len(), 0);
            }
            Err(s) => panic!(),
        }

        let mut too_big: u16 = 0xC000;
        let mut u: Vec<u8> = vec![];
        match u16::encode(&mut too_big, &mut u) {
            Ok(()) => panic!(),
            Err(s) => {}
        }

        let mut n = 0x80;
        let mut u: Vec<u8> = vec![];
        u16::encode(&mut n, &mut u);
        assert_eq!(u.len(), 2);
        assert_eq!(u[0], 0x80);
        assert_eq!(u[1], 0x01);
        match u16::decode(&u[0..]) {
            Ok((m, v)) => {
                assert_eq!(m, 0x80);
                assert_eq!(v.len(), 0);
            }
            Err(e) => panic!(),
        }

        let mut n = 0x1300;
        let mut u: Vec<u8> = vec![];
        u16::encode(&mut n, &mut u);
        assert_eq!(u.len(), 2);
        assert_eq!(u[1], 0x13 << 1);
        assert_eq!(u[0], 0x80);
        match u16::decode(&u[0..]) {
            Ok((m, v)) => {
                assert_eq!(m, 0x1300);
                assert_eq!(v.len(), 0);
            }
            Err(e) => panic!(),
        }

        let mut n = 0x1381;
        let mut u: Vec<u8> = vec![];
        u16::encode(&mut n, &mut u);
        assert_eq!(u.len(), 2);
        assert_eq!(u[1], (0x13 << 1) | 1);
        assert_eq!(u[0], 0x81);
        match u16::decode(&u[0..]) {
            Ok((m, v)) => {
                assert_eq!(m, 0x1381);
                assert_eq!(v.len(), 0);
            }
            Err(e) => panic!(),
        }
    }

    #[test]
    fn message_codec() {
        let mut onward_addresses: Vec<Address> = vec![];
        onward_addresses.push(Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            0x8080,
        ));
        onward_addresses.push(Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 10)),
            0x7070,
        ));
        onward_addresses.push(Address::LocalAddress(
            AddressType::Local,
            LocalAddress {
                address: 0x00010203,
            },
        ));
        let mut return_addresses: Vec<Address> = vec![];
        return_addresses.push(Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            0x8080,
        ));
        return_addresses.push(Address::UdpAddress(
            AddressType::Udp,
            IpAddr::V4(Ipv4Addr::new(10, 0, 1, 11)),
            0x7070,
        ));
        return_addresses.push(Address::LocalAddress(
            AddressType::Local,
            LocalAddress {
                address: 0x00010203,
            },
        ));
        let onward_route = Route {
            addresses: onward_addresses,
        };
        let return_route = Route {
            addresses: return_addresses,
        };
        let mut message_body = vec![0];
        let mut msg = Message {
            onward_route,
            return_route,
            message_body,
        };
        let mut u: Vec<u8> = vec![];
        Message::encode(&mut msg, &mut u);
        assert_eq!(
            u,
            vec![
                3, 2, 0, 127, 0, 0, 1, 0x80, 0x80, 2, 0, 10, 0, 1, 10, 0x70, 0x70, 0, 3, 2, 1, 0,
                3, 2, 0, 127, 0, 0, 2, 0x80, 0x80, 2, 0, 10, 0, 1, 11, 0x70, 0x70, 0, 3, 2, 1, 0,
                0
            ]
        );

        match Message::decode(&u) {
            Ok((m, v)) => {
                assert_eq!(m.onward_route.addresses.len(), 3);
                assert_eq!(
                    m.onward_route.addresses[0],
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        0x8080
                    )
                );
                assert_eq!(
                    m.onward_route.addresses[1],
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(10, 0, 1, 10)),
                        0x7070
                    )
                );
                assert_eq!(
                    m.onward_route.addresses[2],
                    Address::LocalAddress(
                        AddressType::Local,
                        LocalAddress {
                            address: 0x00010203
                        }
                    )
                );
                assert_eq!(m.return_route.addresses.len(), 3);
                assert_eq!(
                    m.return_route.addresses[0],
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
                        0x8080
                    )
                );
                assert_eq!(
                    m.return_route.addresses[1],
                    Address::UdpAddress(
                        AddressType::Udp,
                        IpAddr::V4(Ipv4Addr::new(10, 0, 1, 11)),
                        0x7070
                    )
                );
                assert_eq!(
                    m.return_route.addresses[2],
                    Address::LocalAddress(
                        AddressType::Local,
                        LocalAddress {
                            address: 0x00010203
                        }
                    )
                );
                assert_eq!(m.message_body[0], 0);
            }
            Err(e) => panic!(),
        }
    }
}
