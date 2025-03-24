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
    location: LocationDTO,
}

impl LocationActor {
    pub fn new(location_id: String) -> Self {
        return LocationActor {
            modification_count: 0,
            location: LocationDTO {
                id: "".to_string(),
                seismic_activity: 0.0,
                temperature_c: 0.0,
                radiation_level: 0.0,
                location_id
            }
        }
    }
}

impl Actor for LocationActor {
    type Context = Context<Self>;
}

use actix::prelude::*;
use serde::de::Unexpected::Option;
use crate::dto::LocationDTO;
