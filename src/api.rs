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
use log::info;
use tonic::transport::Channel;
use tonic::{IntoRequest, Request, Status};
use crate::conn_manager::{ChannelManager, GetAllChannels, ResetChannel};
use crate::dto::{EnrichedLocationStats, LocationStats, ShardError};
use crate::location_actor::{GetLocation, GetShard, PutLocation};
use crate::root_actor::{GetAddr, RootActor};
use crate::rs;
use crate::rs::rs::{GetShardRequest, GetShardResponse, WriteShardRequest};


async fn read(addr: Arc<Addr<ChannelManager>>, data: EnrichedLocationStats) -> Result<Vec<()>, ShardError>{
    let channels_result = addr
        .send(GetAllChannels {})
        .await;

    let (current_node, channels) = channels_result.unwrap().unwrap();


    let shards = data.to_shards()?;

    // Generate Reed-Solomon recovery shards
    let recovery_shards = reed_solomon_simd::encode(4, 6, shards.as_slice()).unwrap();


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
            let write_future = write_shard_to_node(
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
        let res = try_join_all(f).boxed().await;
    }



    HttpResponse::NotFound().json(())
}


pub async fn bootstrap(current_node: u32, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let root_actor = Data::new(RootActor::new().start());
    let channel_manager = Data::new(ChannelManager::new(current_node, Duration::from_millis(300)).start());
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

