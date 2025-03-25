use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use actix::prelude::*;
use actix_async_handler::async_handler;
use tokio::sync::Mutex;
use tonic::client::GrpcService;
use tonic::transport::{Channel, Endpoint, Uri};

use actix::prelude::*;

pub struct RootActor {
    addrs: HashMap<String, Addr<LocationActor>>,
}

impl RootActor {
    pub fn new() -> Self {
        return RootActor {
            addrs: HashMap::new(),
        }
    }
}

impl Actor for RootActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("RootActor started");
    }
}

use actix::prelude::*;
use log::{info, log};
use crate::location_actor::LocationActor;

#[derive(Message)]
#[rtype(result = "Result<Addr<LocationActor>, ()>")]
pub struct GetAddr(pub String);


impl Handler<GetAddr> for RootActor {
    type Result = Result<Addr<LocationActor>, ()>;

    fn handle(&mut self, msg: GetAddr, _ctx: &mut Context<Self>) -> Self::Result {
        return if let Some(addr) = self.addrs.get(msg.0.as_str()) {
            println!("actor found for location_id: '{}'\n", msg.0);
            Ok(addr.clone())
        } else {
            println!("creating actor location_id: '{}'\n", msg.0);
            let addr = LocationActor::new(msg.0.clone()).start();
            self.addrs.insert(msg.0, addr.clone());
            Ok(addr)
        }
    }
}
