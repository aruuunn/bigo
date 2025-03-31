use std::collections::HashMap;
use std::time::{Duration, Instant};
use actix::prelude::*;
use tonic::transport::{Channel, Endpoint};
use log::info;

// Message to get a channel for a node
#[derive(Message)]
#[rtype(result = "Result<Channel, String>")]
pub struct GetChannel(pub String); // String is the node ID

// Message to reset a channel for a node
#[derive(Message)]
#[rtype(result = "()")]
pub struct ResetChannel(pub String); // String is the node ID

// Message to get all channels
#[derive(Message)]
#[rtype(result = "Result<HashMap<String, Channel>, ()>")]
pub struct GetAllChannels;

pub struct ChannelManager {
    channels: HashMap<String, Channel>,
    endpoints: HashMap<String, String>, // node_id -> endpoint URL
    reset_timers: HashMap<String, Instant>, // For debouncing
    debounce_duration: Duration,
}

impl ChannelManager {
    pub fn new(debounce_duration: Duration) -> Self {
        ChannelManager {
            channels: HashMap::new(),
            endpoints: HashMap::new(),
            reset_timers: HashMap::new(),
            debounce_duration,
        }
    }

    // Register an endpoint for a node
    pub fn register_endpoint(&mut self, node_id: String, endpoint: String) {
        self.endpoints.insert(node_id, endpoint);
    }

    // Get or create a lazy channel
    fn get_or_create_lazy_channel(&mut self, node_id: &str) -> Result<Channel, String> {
        // Return existing channel if available
        if let Some(channel) = self.channels.get(node_id) {
            return Ok(channel.clone());
        }
        
        // Create a new lazy channel
        if let Some(endpoint_url) = self.endpoints.get(node_id) {
            match Endpoint::from_shared(endpoint_url.clone()) {
                Ok(endpoint) => {
                    // Create a lazily-connected channel
                    let channel = endpoint.connect_lazy();
                    
                    // Store the channel
                    self.channels.insert(node_id.to_string(), channel.clone());
                    Ok(channel)
                },
                Err(e) => Err(format!("Invalid endpoint URL: {}", e))
            }
        } else {
            Err(format!("No endpoint registered for node: {}", node_id))
        }
    }
}

impl Actor for ChannelManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("ChannelManager actor started");
    }
}

impl Handler<GetChannel> for ChannelManager {
    type Result = Result<Channel, String>;

    fn handle(&mut self, msg: GetChannel, _ctx: &mut Context<Self>) -> Self::Result {
        let node_id = msg.0;
        self.get_or_create_lazy_channel(&node_id)
    }
}

impl Handler<ResetChannel> for ChannelManager {
    type Result = ();

    fn handle(&mut self, msg: ResetChannel, ctx: &mut Context<Self>) -> Self::Result {
        let node_id = msg.0;
        let now = Instant::now();
        
        // Implement debouncing
        if let Some(last_reset) = self.reset_timers.get(&node_id) {
            // If we've already reset this channel recently, schedule a delayed reset
            if now.duration_since(*last_reset) < self.debounce_duration {
                info!("Debouncing ResetChannel for node: {}", node_id);
                return;
            }
        }
        
        // If we get here, we're either resetting for the first time, or after the debounce period
        info!("Resetting channel for node: {}", node_id);
        self.channels.remove(&node_id);
        self.reset_timers.insert(node_id, now);
    }
}

// Handler for GetAllChannels
impl Handler<GetAllChannels> for ChannelManager {
    type Result = Result<HashMap<String, Channel>,()>;

    fn handle(&mut self, _msg: GetAllChannels, _ctx: &mut Context<Self>) -> Self::Result {
        Ok(self.channels.clone())
    }
}

// Add/Update an endpoint
#[derive(Message)]
#[rtype(result = "()")]
pub struct RegisterEndpoint {
    pub node_id: String,
    pub endpoint: String,
}

impl Handler<RegisterEndpoint> for ChannelManager {
    type Result = ();

    fn handle(&mut self, msg: RegisterEndpoint, _ctx: &mut Context<Self>) -> Self::Result {
        self.register_endpoint(msg.node_id, msg.endpoint);
    }
}

#[cfg(test)]
mod tests {
    use crate::root_actor::RootActor;

    use super::*;
    use actix::Actor;
    use std::time::Duration;
    use tokio::time::sleep;
    use actix::System;

    // Helper function to create a test endpoint URL
fn test_endpoint(port: u16) -> String {
        format!("http://127.0.0.1:{}", port)
    }

    #[actix::test]
    async fn test_get_channel_happy_path() {
        // Start the ChannelManager actor
        let channel_manager = ChannelManager::new(Duration::from_millis(100)).start();
        
        // Register an endpoint
        let node_id = "test-node-1".to_string();
        let endpoint_url = test_endpoint(50051);
        channel_manager.send(RegisterEndpoint {
            node_id: node_id.clone(),
            endpoint: endpoint_url.clone(),
        }).await.unwrap();
        
        // Get a channel
        let result = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
        assert!(result.is_ok(), "Should successfully create a lazy channel");
        
        // Get the same channel again (should reuse the existing one)
        let result2 = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
        assert!(result2.is_ok(), "Should return the existing channel");
    }


    // #[actix::test]
    // async fn test_reset_channel() {
    //     // Start the ChannelManager actor
    //     let channel_manager = ChannelManager::new(Duration::from_millis(100)).start();
        
    //     // Register an endpoint
    //     let node_id = "reset-test-node".to_string();
    //     channel_manager.send(RegisterEndpoint {
    //         node_id: node_id.clone(),
    //         endpoint: test_endpoint(50052),
    //     }).await.unwrap();
        
    //     // Get a channel
    //     let _ = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
        
    //     // Verify the channel exists using GetAllChannels
    //     let channels = channel_manager.send(GetAllChannels).await.unwrap().unwrap();
    //     assert!(channels.contains_key(&node_id), "Channel should exist before reset");
        
    //     // Reset the channel
    //     channel_manager.send(ResetChannel(node_id.clone())).await.unwrap();
        
    //     // Verify the channel is removed
    //     let channels = channel_manager.send(GetAllChannels).await.unwrap().unwrap();
    //     assert!(!channels.contains_key(&node_id), "Channel should be removed after reset");
    // }

    // #[actix::test]
    // async fn test_debounce_reset_channel() {
    //     // Start the ChannelManager actor with a 200ms debounce time
    //     let channel_manager = ChannelManager::new(Duration::from_millis(200)).start();
        
    //     // Register an endpoint
    //     let node_id = "debounce-test-node".to_string();
    //     channel_manager.send(RegisterEndpoint {
    //         node_id: node_id.clone(),
    //         endpoint: test_endpoint(50053),
    //     }).await.unwrap();
        
    //     // Get a channel
    //     let _ = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
        
    //     // Send multiple reset requests rapidly
    //     for _ in 0..5 {
    //         channel_manager.send(ResetChannel(node_id.clone())).await.unwrap();
    //         sleep(Duration::from_millis(50)).await;
    //     }
        
    //     // Immediately check - channel should still exist due to debouncing
    //     let channels = channel_manager.send(GetAllChannels).await.unwrap().unwrap();
    //     assert!(channels.contains_key(&node_id), "Channel should still exist during debounce period");
        
    //     // Wait for debounce period to complete
    //     sleep(Duration::from_millis(250)).await;
        
    //     // Now the channel should be removed
    //     let channels = channel_manager.send(GetAllChannels).await.unwrap().unwrap();
    //     assert!(!channels.contains_key(&node_id), "Channel should be removed after debounce period");
    // }

    #[actix::test]
    async fn test_get_all_channels() {
        // Start the ChannelManager actor
        let channel_manager = ChannelManager::new(Duration::from_millis(100)).start();
        
        // Register multiple endpoints
        let node_ids = vec!["node-1", "node-2", "node-3"];
        for (i, node_id) in node_ids.iter().enumerate() {
            channel_manager.send(RegisterEndpoint {
                node_id: node_id.to_string(),
                endpoint: test_endpoint(50060 + i as u16),
            }).await.unwrap();
            
            // Create channels for each node
            let _ = channel_manager.send(GetChannel(node_id.to_string())).await.unwrap();
        }
        
        // Get all channels
        let channels = channel_manager.send(GetAllChannels).await.unwrap().unwrap();
        
        // Verify all channels exist
        assert_eq!(channels.len(), node_ids.len(), "Should have channels for all registered nodes");
        for node_id in node_ids {
            assert!(channels.contains_key(node_id), "Should have a channel for {}", node_id);
        }
    }

    // #[actix::test]
    // async fn test_register_and_update_endpoint() {
    //     // Start the ChannelManager actor
    //     let channel_manager = ChannelManager::new(Duration::from_millis(100)).start();
        
    //     // Register an endpoint
    //     let node_id = "update-test-node".to_string();
    //     let original_endpoint = test_endpoint(50070);
    //     channel_manager.send(RegisterEndpoint {
    //         node_id: node_id.clone(),
    //         endpoint: original_endpoint.clone(),
    //     }).await.unwrap();
        
    //     // Get a channel with the original endpoint
    //     let _ = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
        
    //     // Reset the channel
    //     channel_manager.send(ResetChannel(node_id.clone())).await.unwrap();
        
    //     // Update the endpoint
    //     let new_endpoint = test_endpoint(50071);
    //     channel_manager.send(RegisterEndpoint {
    //         node_id: node_id.clone(),
    //         endpoint: new_endpoint.clone(),
    //     }).await.unwrap();
        
    //     // Get a channel with the new endpoint
    //     let result = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
    //     assert!(result.is_ok(), "Should successfully create a new channel with updated endpoint");
    // }

    // Test integration with RootActor
    // #[actix::test]
    // async fn test_root_actor_integration() {
    //     let system = System::new();
        
    //     system.block_on(async {
    //         // Start the RootActor
    //         let root_actor = RootActor::new().start();
            
    //         // Get the ChannelManager from RootActor
    //         let channel_manager_result = root_actor.send(GetChannelManager).await.unwrap();
    //         assert!(channel_manager_result.is_ok(), "Should successfully get ChannelManager from RootActor");
            
    //         let channel_manager = channel_manager_result.unwrap();
            
    //         // Register an endpoint
    //         let node_id = "root-integration-node".to_string();
    //         channel_manager.send(RegisterEndpoint {
    //             node_id: node_id.clone(),
    //             endpoint: test_endpoint(50080),
    //         }).await.unwrap();
            
    //         // Get a channel
    //         let result = channel_manager.send(GetChannel(node_id.clone())).await.unwrap();
    //         assert!(result.is_ok(), "Should successfully get channel through RootActor integration");
    //     });
    // }
}