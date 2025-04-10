use std::env;
use std::net::IpAddr;
use std::str::FromStr;
use std::error::Error;
use actix_web::{web, App, HttpResponse, HttpServer};
use actix_web::{get, HttpRequest, Responder};
mod api;
mod root_actor;
mod location_actor;
mod dto;
mod rs;
mod conn_manager;
fn parse_ip(ip_str: &str) -> IpAddr {
    return IpAddr::from_str(ip_str.trim()).unwrap();
}


#[actix::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env::set_var("CURRENT_NODE_IP", "172.0.0.2");
    env::set_var("ALL_NODE_IPS", "172.0.0.2,127.0.0.1");
    let current_node_ip = parse_ip(&env::var("CURRENT_NODE_IP")?);
    let all_node_ips: Vec<_> =  env::var("ALL_NODE_IPS").unwrap()
                .split(',')
                .map(|ip_str| parse_ip(ip_str))
                .collect();

    api::bootstrap(8080).await?;
    Ok(())
}

