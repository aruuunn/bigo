use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use actix::prelude::*;
use actix_async_handler::async_handler;
use tokio::sync::Mutex;
use tonic::client::GrpcService;
use tonic::transport::{Channel, Endpoint, Uri};
use actix::prelude::*;
use log::{info, log};

use crate::location_actor::LocationActor;

const INITIAL_POOL_SIZE: usize = 6000;


pub struct RootActor {
    addrs: HashMap<String, Addr<LocationActor>>,
    pool: VecDeque<Addr<LocationActor>>,
}


impl RootActor {
    pub fn new() -> Self {
        let mut warm_pool = VecDeque::with_capacity(INITIAL_POOL_SIZE);
        for _ in 0..INITIAL_POOL_SIZE {
            warm_pool.push_back(LocationActor::new().start());
        }
        return RootActor {
            addrs: HashMap::new(),
            pool: warm_pool
        }
    }
}

impl Actor for RootActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("RootActor started");
    }
}



#[derive(Message)]
#[rtype(result = "Result<Addr<LocationActor>, ()>")]
pub struct GetAddr(pub String);

impl Handler<GetAddr> for RootActor {
    type Result = Result<Addr<LocationActor>, ()>;

    fn handle(&mut self, msg: GetAddr, _ctx: &mut Context<Self>) -> Self::Result {
        return if let Some(addr) = self.addrs.get(msg.0.as_str()) {
            info!("actor found for location_id: '{}'\n", msg.0);
            Ok(addr.clone())
        } else {
            info!("creating actor location_id: '{}'\n", msg.0);
            let addr = self.pool.pop_front().map(|addr| {
                addr
            }).unwrap_or_else(|| {
                info!("pool is empty, creating new actor for location_id: '{}'\n", msg.0);
                LocationActor::new().start()
            });

            self.addrs.insert(msg.0, addr.clone());
            Ok(addr)
        }
    }
}
