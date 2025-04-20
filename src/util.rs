use std::net::SocketAddr;

pub fn parse_socket_addr(addr_str: &str) -> Result<(String, u16), Box<dyn std::error::Error>> {
    let socket_addr: SocketAddr = addr_str.parse()?;
    let ip = socket_addr.ip().to_string();
    let port = socket_addr.port();
    
    Ok((ip, port))
}
