use std::net::SocketAddr;

pub fn parse_socket_addr(addr_str: &str) -> Result<(String, u16), Box<dyn std::error::Error>> {
    let socket_addr: SocketAddr = addr_str.parse()?;
    let ip = socket_addr.ip().to_string();
    let port = socket_addr.port();
    
    Ok((ip, port))
}

pub fn get_owner_node_id(location_id: String) -> u32 {
    let mut owner_id = 0;
    for (_, c) in location_id.chars().enumerate() {
        owner_id = ((owner_id * 10) % 7 + (c as u32) % 7) % 7;
    }
   return owner_id;
}
