use crate::node::Node;
use actix_web::{get, HttpRequest, Responder};
use actix_web::{web, App, HttpResponse, HttpServer};
use std::env;
use std::error::Error;
use std::net::IpAddr;
use std::str::FromStr;
use tokio::join;
use tonic::transport::Server;

mod api;
mod node;
mod rs;
mod conn_manager;
mod root_actor;
mod location_actor;
mod dto;

fn parse_ip(ip_str: &str) -> IpAddr {
    return IpAddr::from_str(ip_str.trim()).unwrap();
}

#[actix::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env::set_var("CURRENT_NODE_IP", "172.0.0.2");
    env::set_var("ALL_NODE_IPS", "172.0.0.2,127.0.0.1");
    let current_node_ip = parse_ip(&env::var("CURRENT_NODE_IP")?);
    let all_node_ips: Vec<_> = env::var("ALL_NODE_IPS")
        .unwrap()
        .split(',')
        .map(|ip_str| parse_ip(ip_str))
        .collect();

    let current_node_idx = all_node_ips.iter().position(|ip| ip == &current_node_ip).unwrap();

    let addr = "127.0.0.1:3000".parse()?;
    let node = Node::default();

    join!(
        Server::builder()
            .add_service(rs::rs::rs_server::RsServer::new(node))
            .serve(addr),
        api::bootstrap(8080)
    )?;

    Ok(())
}
