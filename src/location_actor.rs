use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use actix::prelude::*;
use actix_async_handler::async_handler;
use tokio::sync::Mutex;
use tonic::client::GrpcService;
use tonic::transport::{Channel, Endpoint, Uri};

use actix::prelude::*;

pub struct LocationActor {
    modification_count: i64,
    location_id: String,
    location: LocationStats,
}

impl LocationActor {
    pub fn new(location_id: String) -> Self {
        return LocationActor {
            modification_count: 0,
            location_id,
            location: LocationStats {
                id: "".to_string(),
                seismic_activity: 0.0,
                temperature_c: 0.0,
                radiation_level: 0.0,
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct PutLocation(pub LocationStats);

#[derive(Message)]
#[rtype(result = "Result<EnrichedLocationStats, ()>")]
pub struct GetLocation;

impl Actor for LocationActor {
    type Context = Context<Self>;
}

impl Handler<PutLocation> for LocationActor {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: PutLocation, _ctx: &mut Self::Context) -> Self::Result {
        self.modification_count += 1;
        self.location = msg.0;
        Ok(())
    }
}

impl Handler<GetLocation> for LocationActor {
    type Result = Result<EnrichedLocationStats, ()>;
    fn handle(&mut self, msg: GetLocation, _ctx: &mut Self::Context) -> Self::Result {
        Ok(EnrichedLocationStats::from(self.modification_count, self.location.clone()))
    }
}

use actix::prelude::*;
use serde::de::Unexpected::Option;
use crate::dto::{EnrichedLocationStats, LocationStats};
