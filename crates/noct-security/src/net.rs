//! Controlli di rete senza I/O, condivisi dai moduli che contattano host scelti dagli utenti
//! (anteprima dei link, sincronizzazione Git): quali indirizzi non vanno mai raggiunti.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

fn v4_blocked(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || o[0] == 0
        || o[0] >= 240 // riservato
        || (o[0] == 100 && (64..128).contains(&o[1])) // CGNAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // IETF
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19)) // benchmark
        || (o[0] == 192 && o[1] == 0 && o[2] == 2)
        || (o[0] == 198 && o[1] == 51 && o[2] == 100)
        || (o[0] == 203 && o[1] == 0 && o[2] == 113) // documentazione
}

fn v6_blocked(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_blocked(v4);
    }
    let seg = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (seg[0] & 0xfe00) == 0xfc00 // unique local
        || (seg[0] & 0xffc0) == 0xfe80 // link local
        || (seg[0] == 0x2001 && seg[1] == 0x0db8) // documentazione
        || (seg[0] == 0x0064 && seg[1] == 0xff9b) // NAT64
}

/// `true` per gli indirizzi che non devono mai essere raggiunti da un fetch pubblico.
pub fn is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_blocked(v4),
        IpAddr::V6(v6) => v6_blocked(v6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn private_and_reserved_addresses_are_blocked() {
        for s in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.5.4",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "198.18.0.1",
            "::1",
            "::",
            "fe80::1",
            "fc00::1",
            "fd12::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "2001:db8::1",
        ] {
            assert!(is_blocked(ip(s)), "{s} deve essere bloccato");
        }
        for s in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700:4700::1111", "::ffff:8.8.8.8"] {
            assert!(!is_blocked(ip(s)), "{s} è pubblico");
        }
    }
}
