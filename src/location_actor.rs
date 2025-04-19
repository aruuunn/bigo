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
    location: Option<Box<LocationStats>>,
    shard: Option<Box<Vec<u8>>>,
}

impl LocationActor {
    pub fn new(location_id: String) -> Self {
        return LocationActor {
            modification_count: 0,
            location_id,
            location: None,
            shard: None,
        }
    }

    async fn write(self: &Self, addr: Arc<Addr<ChannelManager>>, data: EnrichedLocationStats) -> Result<Vec<()>, ShardError>{
        let channels_result = addr
            .send(GetAllChannels {})
            .await;

        let (current_node, channels) = channels_result.unwrap().unwrap();


        let shards = data.to_shards()?;

        // Generate Reed-Solomon recovery shards
        let recovery_shards = [shards.to_vec(), reed_solomon_simd::encode(4, 2, shards.as_slice()).unwrap()].concat();


        // Send shards to other nodes
        let mut futures = Vec::new();


        for (i, shard) in recovery_shards.into_iter().enumerate() {
            // Skip self-sharding if needed

            // Get node_id for this shard (using some strategy, e.g., consistent hashing)


            let node_id: u32 = if i as u32 >= current_node {
                i+1
            } else {
                i
            } as u32;

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
                (println!("No channel found for node {}", node_id));
            }
        }

        try_join_all(futures).boxed().await
    }

    async fn write_shard_to_node(
        &self,
        addr: Arc<Addr<ChannelManager>>,
        node_id: u32,
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

// New message types for shard operations
#[derive(Message)]
#[rtype(result = "Result<(), ShardError>")]
pub struct PutShard(pub Vec<u8>);

#[derive(Message)]
#[rtype(result = "Result<Vec<u8>, ShardError>")]
pub struct GetShard;

impl Actor for LocationActor {
    type Context = Context<Self>;
}



impl Handler<PutLocation> for LocationActor {
    type Result = AtomicResponse<Self, Result<Vec<()>, ShardError>>;
     
    fn handle(&mut self, msg: PutLocation, _ctx: &mut Self::Context) -> Self::Result {
        self.modification_count += 1;
        self.location = Some(Box::new(msg.0.clone()));
        let data = EnrichedLocationStats::from(self.modification_count, msg.0);
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
    
    fn handle(&mut self, _msg: GetLocation, _ctx: &mut Self::Context) -> Self::Result {
        match &self.location {
            Some(location) => {
                Ok(EnrichedLocationStats::from(self.modification_count, (**location).clone()))
            },
            None => Err(())
        }
    }
}


// Handler for PutShard message
impl Handler<PutShard> for LocationActor {
    type Result = Result<(), ShardError>;
    
    fn handle(&mut self, msg: PutShard, _ctx: &mut Self::Context) -> Self::Result {
        self.shard = Some(Box::new(msg.0));
        Ok(())
    }
}

// Handler for GetShard message
impl Handler<GetShard> for LocationActor {
    type Result = Result<Vec<u8>, ShardError>;
    
    fn handle(&mut self, _msg: GetShard, _ctx: &mut Self::Context) -> Self::Result {
        match &self.shard {
            Some(shard) => Ok((**shard).clone()),
            None => Err(ShardError::NotFoundError(format!("Shard not found for location {}", self.location_id)))
        }
    }
}


use tonic::{Request, Status};
use crate::conn_manager::{ChannelManager, GetAllChannels, ResetChannel};
use crate::dto::{EnrichedLocationStats, LocationStats, ShardError};
use crate::rs;
use crate::rs::rs::WriteShardRequest;

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use std::time::Duration;
    use std::sync::Arc;
    use tokio::time::timeout;

    // Mock ChannelManager for testing
    #[derive(Default)]
    struct MockChannelManager;

    impl Actor for MockChannelManager {
        type Context = Context<Self>;
    }

    impl Handler<GetAllChannels> for MockChannelManager {
        type Result = Result<(u32, HashMap<u32, Channel>), ()>;

        fn handle(&mut self, _msg: GetAllChannels, _ctx: &mut Self::Context) -> Self::Result {
            Ok((0, HashMap::new()))
        }
    }

    impl Handler<ResetChannel> for MockChannelManager {
        type Result = ();

        fn handle(&mut self, _msg: ResetChannel, _ctx: &mut Self::Context) -> Self::Result {
            ()
        }
    }

    #[actix::test]
    async fn test_put_shard_and_get_shard() {
        // Arrange
        let location_id = "test_location_1".to_string();
        let test_shard_data = vec![1, 2, 3, 4, 5];
        
        // Create actor
        let addr = LocationActor::new(location_id).start();
        
        // Act - Put the shard data
        let put_result = addr.send(PutShard(test_shard_data.clone())).await.unwrap();
        
        // Assert - Put should succeed
        assert!(put_result.is_ok());
        
        // Act - Get the shard data
        let get_result = addr.send(GetShard).await.unwrap();
        
        // Assert - Get should return the same data
        match get_result {
            Ok(returned_data) => {
                assert_eq!(returned_data, test_shard_data);
            },
            Err(e) => {
                panic!("Expected successful GetShard, got error: {:?}", e);
            }
        }
    }

    #[actix::test]
    async fn test_get_shard_when_not_initialized() {
        // Arrange - Create actor without putting a shard
        let location_id = "test_location_2".to_string();
        let addr = LocationActor::new(location_id).start();
        
        // Act - Try to get a shard that doesn't exist
        let get_result = addr.send(GetShard).await.unwrap();
        
        // Assert - Should get NotFound error
        match get_result {
            Ok(_) => {
                panic!("Expected error, got successful response");
            },
            Err(error) => {
                // Check if it's the expected NotFound error
                if let ShardError::NotFoundError(_) = error {
                    // This is the expected path
                    assert!(true);
                } else {
                    panic!("Expected NotFound error, got: {:?}", error);
                }
            }
        }
    }

    #[actix::test]
    async fn test_multiple_put_shard_operations() {
        // Arrange
        let location_id = "test_location_3".to_string();
        let first_shard = vec![1, 2, 3];
        let second_shard = vec![4, 5, 6];
        
        // Create actor
        let addr = LocationActor::new(location_id).start();
        
        // Act - Put first shard
        let put_result1 = addr.send(PutShard(first_shard)).await.unwrap();
        assert!(put_result1.is_ok());
        
        // Get first shard
        let get_result1 = addr.send(GetShard).await.unwrap();
        assert_eq!(get_result1.unwrap(), vec![1, 2, 3]);
        
        // Put second shard
        let put_result2 = addr.send(PutShard(second_shard)).await.unwrap();
        assert!(put_result2.is_ok());
        
        // Get updated shard
        let get_result2 = addr.send(GetShard).await.unwrap();
        assert_eq!(get_result2.unwrap(), vec![4, 5, 6]);
    }


    #[actix::test]
    async fn test_large_shard_data() {
        // Arrange - Create actor
        let location_id = "test_location_6".to_string();
        let addr = LocationActor::new(location_id).start();
        
        // Create large shard data (1MB)
        let large_shard = vec![0u8; 1_000_000];
        
        // Act - Put and get large shard
        let put_result = addr.send(PutShard(large_shard.clone())).await.unwrap();
        assert!(put_result.is_ok());
        
        let get_result = addr.send(GetShard).await.unwrap();
        
        // Assert
        let returned_data = get_result.unwrap();
        assert_eq!(returned_data.len(), 1_000_000);
        assert_eq!(returned_data, large_shard);
    }
}
