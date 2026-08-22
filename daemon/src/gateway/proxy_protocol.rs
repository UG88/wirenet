use std::net::{IpAddr, SocketAddr};

const PROXY_V2_HEADER_PREFIX: &[u8; 12] = b"\r\n\r\n\x00\r\nQUIT\n";

/// Generates a binary PROXY Protocol v2 header for an incoming connection
pub fn encode_proxy_v2_header(src: SocketAddr, dst: SocketAddr) -> Vec<u8> {
    let mut header = Vec::with_capacity(32);
    header.extend_from_slice(PROXY_V2_HEADER_PREFIX);

    // Version 2, Command: PROXY (0x21)
    header.push(0x21);

    match (src.ip(), dst.ip()) {
        (IpAddr::V4(src_v4), IpAddr::V4(dst_v4)) => {
            // Family: AF_INET, Protocol: STREAM (TCP) = 0x11
            header.push(0x11);
            // Length of addresses and ports = 12 bytes (4 + 4 + 2 + 2)
            header.extend_from_slice(&12u16.to_be_bytes());

            header.extend_from_slice(&src_v4.octets());
            header.extend_from_slice(&dst_v4.octets());
            header.extend_from_slice(&src.port().to_be_bytes());
            header.extend_from_slice(&dst.port().to_be_bytes());
        }
        (IpAddr::V6(src_v6), IpAddr::V6(dst_v6)) => {
            // Family: AF_INET6, Protocol: STREAM (TCP) = 0x21
            header.push(0x21);
            // Length of addresses and ports = 36 bytes (16 + 16 + 2 + 2)
            header.extend_from_slice(&36u16.to_be_bytes());

            header.extend_from_slice(&src_v6.octets());
            header.extend_from_slice(&dst_v6.octets());
            header.extend_from_slice(&src.port().to_be_bytes());
            header.extend_from_slice(&dst.port().to_be_bytes());
        }
        _ => {
            // Fallback UNSPEC (0x00)
            header.push(0x00);
            header.extend_from_slice(&0u16.to_be_bytes());
        }
    }

    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_v2_ipv4_encoding() {
        let src = "192.168.1.100:12345".parse().unwrap();
        let dst = "10.0.0.1:25565".parse().unwrap();

        let header = encode_proxy_v2_header(src, dst);

        assert_eq!(&header[0..12], b"\r\n\r\n\x00\r\nQUIT\n");
        assert_eq!(header[12], 0x21); // Ver 2, PROXY
        assert_eq!(header[13], 0x11); // AF_INET, STREAM
        assert_eq!(header.len(), 28); // 12 + 4 + 12
    }
}
