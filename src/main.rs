use std::env;
use std::net::IpAddr;
use std::str::FromStr;
use std::error::Error;
use actix_web::{web, App, HttpResponse, HttpServer};
use actix_web::{get, HttpRequest, Responder};
use log::logger;
use util::parse_socket_addr;
mod api;
mod root_actor;
mod location_actor;
mod dto;
mod rs;
mod node;
mod conn_manager;
mod util;
mod constants;




#[actix_rt::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let current_node_ip_value = env::var("CURRENT_NODE_IP").unwrap();
    let current_node_ip = current_node_ip_value.trim();
    let all_node_ips: Vec<String> =  env::var("ALL_NODE_IPS").unwrap()
                .trim()
                .split(',')
                .map(|ip_str| ip_str.trim().to_owned())
                .collect();
    
    let current_node_idx = all_node_ips.iter()
    .position(|ip| *ip == current_node_ip)
    .expect("Current node IP not found in ALL_NODE_IPS");

    println!("Current node index: {}", current_node_idx);

    api::bootstrap(current_node_idx as u32, all_node_ips, parse_socket_addr(&current_node_ip_value).unwrap().1).await?;
    Ok(())
}

