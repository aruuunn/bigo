use std::collections::HashMap;
use std::error;
use std::future::{self, Future};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use actix::{Actor, Addr};
use actix_web::{put, web, App, HttpResponse, HttpServer};
use actix_web::{get, HttpRequest, Responder};
use actix_web::dev::Path;
use actix_web::test::status_service;
use actix_web::web::{Data, Json};
use futures::future::{try_join, try_join_all};
use futures::{join, FutureExt};
use log::{error, info};
use tonic::transport::Channel;
use tonic::{IntoRequest, Request, Status};
use crate::conn_manager::{ChannelManager, GetAllChannels, ResetChannel};
use crate::dto::{EnrichedLocationStats, LocationStats, ShardError};
use crate::location_actor::{GetLocation, GetShard, PutLocation};
use crate::root_actor::{GetAddr, RootActor};
use crate::rs;
use crate::rs::rs::{GetShardRequest, GetShardResponse, WriteShardRequest};


fn get_owner_node_id(location_id: String) -> u32 {
   return 1;
}


async fn read_shard_from_node(
    addr: Arc<Addr<ChannelManager>>,
    node_id: u32,
    channel: Channel,
    location_id: String,
) -> Result<GetShardResponse, ShardError> {
    // Create a gRPC client using the channel
    let mut client = rs::rs::rs_client::RsClient::new(channel);
    
    // Prepare the request
    let request = Request::new(GetShardRequest {
        location_id
    });
    
    // Send the RPC call and handle errors
    match client.get_shard_request(request).await {
        Ok(res) => Ok(res.into_inner()),
        Err(status) => {
            // Check if it's a connection error
            if is_connection_error(&status) {
                // Send ResetConnection message to ChannelManager
                addr.do_send(ResetChannel(node_id));
            }
            
            error!("Failed to read shard from node {}: {}", node_id, status);
            Ok(GetShardResponse {
                shard: None,
                location_stats: None,
            })
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


#[get("/ping")]
async fn index(_req: HttpRequest) -> impl Responder {
    "pong"
}

#[put("/{location_id}")]
async fn put(body: Json<LocationStats>, id: web::Path<String>, root_actor: Data<Addr<RootActor>>, channel_manager: Data<Addr<ChannelManager>>) -> impl Responder {
    let location_id = id.into_inner();
   let addr =  root_actor.send(GetAddr(location_id)).await.unwrap().unwrap();
    addr.send(PutLocation(body.into_inner(), channel_manager.into_inner())).await.unwrap().unwrap();
    return HttpResponse::Created()
}

#[get("/{location_id}")]
async fn get(id: web::Path<(String)>, root_actor: Data<Addr<RootActor>>, channel_manager: Data<Addr<ChannelManager>>) -> impl Responder {
    let (location_id) = id.into_inner();
    let addr =  root_actor.send(GetAddr(location_id.clone())).await.unwrap().unwrap();
    if let Ok(location_stats) = addr.send(GetLocation).await.unwrap() {
        return HttpResponse::Ok().json(location_stats);
    }

    if let Ok(shard) = addr.send(GetShard).await.unwrap() {
        let channels_result = channel_manager
        .send(GetAllChannels {})
        .await;

        let (current_node, channels) = channels_result.unwrap().unwrap();

        let mut f = Vec::new();

        for i in 0..7 {
                f.push(read_shard_from_node(channel_manager.clone().into_inner(), i, channels.get(&i).unwrap().clone(), location_id.clone()));
        }
        let res = try_join_all(f).boxed().await.unwrap();
        
        let mut original_shards: HashMap<usize, Vec<u8>> = HashMap::new();
        let mut recovery_shards: HashMap<usize, Vec<u8>> = HashMap::new();

        let owner_node_id = get_owner_node_id(location_id.clone());

        if let Some(location_stats) = res[owner_node_id as usize].location_stats.clone() {
            let enriched_location_stats = EnrichedLocationStats {
                id: location_stats.id,
                seismic_activity: location_stats.seismic_activity,
                temperature_c: location_stats.temperature_c,
                radiation_level: location_stats.radiation_level,
                modification_count: location_stats.modification_count,
            };
            
            return HttpResponse::Ok().json(enriched_location_stats);
        }

        let mut shard_count = 0;

        for i in 0..7usize {
            let node_id = if i as u32 >= owner_node_id {
                i+1
            } else {
                i
            };
            if let Some(shard) = res[node_id as usize].shard.clone() {
                shard_count += 1;
                if i >= 5 {
                    recovery_shards.insert(i, shard);
                } else {
                    // original_shards.push((i, shard));
                    original_shards.insert(i, shard);
                }
          
            }
            //
        }

        if shard_count < 4 {
            return HttpResponse::InternalServerError().json("Not enough shards");
        }

        let restored = reed_solomon_simd::decode(
            4, 2, original_shards.iter()
            .map(|(k, v)| (*k, v.clone())), 
            recovery_shards
            .iter()
            .map(|(k, v)| (*k, v.clone()))
        ).unwrap();

        let mut shards: [Vec<u8>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for i in 0..4 {
            if let Some(data) = original_shards.get(&i) {
                shards[i] = data.clone();
            } else if let Some(data) = restored.get(&i) {
                shards[i] = data.clone();
            } else {
                error!("Failed to restore shard {}", i);
                return HttpResponse::InternalServerError().json("Failed to restore shard");
            }
        }

        return EnrichedLocationStats::from_shards(shards)
            .map(|enriched_location_stats| {
                HttpResponse::Ok().json(enriched_location_stats)
            })
            .unwrap_or_else(|_| {
                error!("Failed to decode shards");
                HttpResponse::InternalServerError().json("Failed to decode shards")
            });
    }


    HttpResponse::NotFound().json(())
}


pub async fn bootstrap(current_node: u32, port: u16, endpoint: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root_actor = Data::new(RootActor::new().start());
    let cm = ChannelManager::new(current_node, endpoint, Duration::from_millis(300)).start();
    
    let channel_manager = Data::new(cm);
    HttpServer::new(move || App::new()
        .app_data(Data::clone(&root_actor))
        .app_data( Data::clone(&channel_manager))
        .service(index)
        .service(put)
        .service(get))
        .bind(("0.0.0.0", port))?
        .run()
        .await?;

    Ok(())
}

