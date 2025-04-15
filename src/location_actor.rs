use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use actix::fut::{ready, wrap_future};
use actix::prelude::*;
use actix_async_handler::async_handler;
use futures::future::try_join_all;
use futures::FutureExt;
use log::info;
use tokio::sync::Mutex;
use tonic::client::GrpcService;
use tonic::transport::{Channel, Endpoint, Uri};

use actix::prelude::*;

#[derive(Clone)]
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

    async fn write(self: &Self, addr: Arc<Addr<ChannelManager>>, data: EnrichedLocationStats) -> Result<Vec<()>, ShardError>{
        let channels_result = addr
            .send(GetAllChannels {})
            .await;

        let channels = channels_result.unwrap().unwrap();


        let shards = data.to_shards()?;

        // Generate Reed-Solomon recovery shards
        let recovery_shards = reed_solomon_simd::encode(4, 6, shards.as_slice()).unwrap();


        // Send shards to other nodes
        let mut futures = Vec::new();


        for (i, shard) in recovery_shards.into_iter().enumerate() {
            // Skip self-sharding if needed

            // Get node_id for this shard (using some strategy, e.g., consistent hashing)
            let node_id = format!("node_{}", i); // This is a placeholder - implement your own strategy

            if let Some(channel) = channels.get(&node_id) {
                let write_future = self.write_shard_to_node(
                    addr.clone(),
                    node_id.clone(),
                    channel.clone(),
                    self.location_id.clone(),
                    shard.to_vec(),
                );
                futures.push(write_future);
            } else {
                (info!("No channel found for node {}", node_id));
            }
        }

        try_join_all(futures).boxed().await
    }

    async fn write_shard_to_node(
        &self,
        addr: Arc<Addr<ChannelManager>>,
        node_id: String,
        channel: Channel,
        location_id: String,
        shard_data: Vec<u8>,
    ) -> Result<(), ShardError> {
        // Create a gRPC client using the channel
        let mut client = rs::rs::rs_client::RsClient::new(channel);
        
        // Prepare the request
        let request = Request::new(WriteShardRequest {
            location_id,
            shard: shard_data,
        });
        
        // Send the RPC call and handle errors
        match client.write_shard_request(request).await {
            Ok(_) => Ok(()),
            Err(status) => {
                // Check if it's a connection error
                if Self::is_connection_error(&status) {
                    // Send ResetConnection message to ChannelManager
                    addr.do_send(ResetChannel(node_id));
                }
                
                Err(ShardError::RpcError(format!("Failed to write shard: {}", status)))
            }
        }
    }
    
    // Helper method to determine if the error is a connection error
    fn is_connection_error(status: &Status) -> bool {
        // Check for common connection errors
        matches!(
            status.code(),
            tonic::Code::Unavailable | 
            tonic::Code::DeadlineExceeded | 
            tonic::Code::Cancelled |
            tonic::Code::Aborted
        )
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<()>,ShardError>")]
pub struct PutLocation(pub LocationStats, pub Arc<Addr<ChannelManager>>);

#[derive(Message)]
#[rtype(result = "Result<EnrichedLocationStats, ()>")]
pub struct GetLocation;

impl Actor for LocationActor {
    type Context = Context<Self>;
}



impl Handler<PutLocation> for LocationActor {
    type Result = AtomicResponse<Self, Result<Vec<()>, ShardError>>;
     
    fn handle(&mut self, msg: PutLocation, _ctx: &mut Self::Context) -> Self::Result {
        self.modification_count += 1;
        self.location = msg.0;
        let data = EnrichedLocationStats::from(self.modification_count, self.location.clone());
        let actor = self.clone();

        return AtomicResponse::new(Box::pin(
            async move { 
                actor.write(msg.1, data).await
            }.into_actor(self)
        ));
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
use tonic::{Request, Status};
use crate::conn_manager::{ChannelManager, GetAllChannels, ResetChannel};
use crate::dto::{EnrichedLocationStats, LocationStats, ShardError};
use crate::rs;
use crate::rs::rs::WriteShardRequest;
