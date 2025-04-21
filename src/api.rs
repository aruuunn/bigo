use std::collections::HashMap;
use std::future::{self, Future};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use actix::{Actor, Addr, Arbiter, SyncArbiter};
use actix_web::{put, web, App, HttpResponse, HttpServer};
use actix_web::{get, HttpRequest, Responder};
use actix_web::dev::Path;
use actix_web::test::status_service;
use actix_web::web::{Data, Json};
use awc::{Client, JsonBody};
use futures::future::{try_join, try_join_all};
use futures::{FutureExt};
use log::{error, info};
use tokio::join;
use tonic::transport::{Channel, Server};
use tonic::{IntoRequest, Request, Status};
use crate::conn_manager::{ChannelManager, GetAllChannels, GetChannel, ResetChannel};
use crate::constants::ROOT_ACTOR_POOL_SIZE;
use crate::dto::{EnrichedLocationStats, ExtendedLocationStats, LocationStats, ShardError};
use crate::location_actor::{GetLocation, GetShard, PutLocation};
use crate::node::Node;
use crate::root_actor::{GetAddr, RootActor};
use crate::rs;
use crate::rs::rs::rs_server::{Rs, RsServer};
use crate::rs::rs::{GetShardRequest, GetShardResponse, RouteWriteRequest, WriteShardRequest};
use crate::util::get_owner_node_id;



async fn read_shard_from_node(
    addr: Arc<Addr<ChannelManager>>,
    node_id: u32,
    channel: Channel,
    location_id: String,
) -> Result<GetShardResponse, ShardError> {
    let mut client = rs::rs::rs_client::RsClient::new(channel);
    
    let request = Request::new(GetShardRequest {
        location_id
    });
    
    match client.get_shard_request(request).await {
        Ok(res) => Ok(res.into_inner()),
        Err(status) => {
            if is_connection_error(&status) {
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

fn is_connection_error(status: &Status) -> bool {
    matches!(
        status.code(),
        tonic::Code::Unavailable | 
        tonic::Code::DeadlineExceeded | 
        tonic::Code::Cancelled |
        tonic::Code::Aborted
    )
}


#[get("/health")]
async fn index(_req: HttpRequest) -> impl Responder {
    "pong"
}

#[put("/{location_id}")]
async fn put(body: Json<LocationStats>, id: web::Path<String>, root_actor_pool: Data<Vec<Addr<RootActor>>>, channel_manager: Data<Addr<ChannelManager>>, current_node: Data<u32>, endpoints: Data<Vec<String>>, client: Data<Client>) -> impl Responder {
    let location_id = id.into_inner();
    let root_actor = root_actor_pool.get((get_owner_node_id(location_id.clone()) % ROOT_ACTOR_POOL_SIZE) as usize).unwrap();
    let owner_id = get_owner_node_id(location_id.clone());
 

    if *current_node.into_inner() != owner_id {
        let channel = channel_manager
        .send(GetChannel(owner_id))
        .await.unwrap().unwrap();
        let mut client = rs::rs::rs_client::RsClient::new(channel.clone());

        match client.route_write(Request::new(RouteWriteRequest {
            location_id: location_id.clone(),
            id: body.id.clone(),
            seismic_activity: body.seismic_activity,
            temperature_c: body.temperature_c,
            radiation_level: body.radiation_level
        })).await {
            Ok(_) => {
                return HttpResponse::Created().json(());
            }
            Err(status) => {
                if is_connection_error(&status) {
                    channel_manager.do_send(ResetChannel(owner_id));
                }
                error!("Failed to route write to node {}: {}", owner_id, status);
                return HttpResponse::InternalServerError().json("Failed to route write");
            }  
            
        }

        // let url = format!("http://{}/{}", endpoints.get(owner_id as usize).unwrap(),location_id);
        // let resp = client
        // .put(&url).send_json(&body.clone()).await;

        // // info!("Sending location stats to {}: {:?}", url, body.id);

        // if let Err(err) = resp {
        //     error!("Failed to put location stats: {} {}", err, location_id);
        //     return HttpResponse::InternalServerError().json("Failed to put location stats");
        // }

        // let result=  resp.unwrap();
        // if result.status() != 201 {
        //     error!("Failed to put location stats: {} {}", result.status(), location_id);
        //     return HttpResponse::InternalServerError().json("Failed to put location stats");
        // }
    
        // return HttpResponse::Created().json(());
    }


   let addr =  root_actor.send(GetAddr(location_id.clone())).await.unwrap().unwrap();
    if let Err(err) = addr.send(PutLocation(ExtendedLocationStats::from_basic(location_id.clone(), body.into_inner()), channel_manager.into_inner())).await.unwrap() {
        error!("Failed to put location stats: {} {}", err, location_id);
        return HttpResponse::InternalServerError().json("Failed to put location stats");
    }

    return HttpResponse::Created().json(());
}

#[get("/{location_id}")]
async fn get(id: web::Path<(String)>, root_actor_pool: Data<Vec<Addr<RootActor>>>, channel_manager: Data<Addr<ChannelManager>>) -> impl Responder {
    let (location_id) = id.into_inner();
    let root_actor = root_actor_pool.get((get_owner_node_id(location_id.clone()) % ROOT_ACTOR_POOL_SIZE) as usize).unwrap();
    let addr =  root_actor.send(GetAddr(location_id.clone())).await.unwrap().unwrap();
    if let Ok(location_stats) = addr.send(GetLocation).await.unwrap() {
        return HttpResponse::Ok().json(location_stats);
    }

    if let Ok(shard) = addr.send(GetShard(location_id.clone())).await.unwrap() {
        let channels_result = channel_manager
        .send(GetAllChannels {})
        .await;

        let (_, channels) = channels_result.unwrap().unwrap();

        let mut f = Vec::new();

        for i in 0..7 {
                f.push(read_shard_from_node(channel_manager.clone().into_inner(), i, channels.get(&i).unwrap().clone(), location_id.clone()));
        }
        let res = try_join_all(f).boxed().await.unwrap();
        
        let mut original_shards: HashMap<usize, Vec<u8>> = HashMap::new();
        let mut recovery_shards: HashMap<usize, Vec<u8>> = HashMap::new();

        let owner_node_id = get_owner_node_id(location_id);

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


        for i in 0..6usize {
            let node_id = if i as u32 >= owner_node_id {
                i+1
            } else {
                i
            };
            if let Some(shard) = res[node_id as usize].shard.clone() {
                shard_count += 1;
                if i >= 4 {
                    recovery_shards.insert(i-4, shard);
                } else {
                    original_shards.insert(i, shard);
                }
          
            }
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


pub async fn bootstrap(current_node: u32,endpoint: Vec<String>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut root_actor_pool: Vec<Addr<RootActor>> = Vec::new();
    for _ in 0..ROOT_ACTOR_POOL_SIZE {
        root_actor_pool.push(SyncArbiter::start(1, move || {
            RootActor::new()
        }));
        // root_actor_pool.push(RootActor::start_in_arbiter(&a.handle(), |_| RootActor::new()));
    }

    let root_actor = Data::new(root_actor_pool.clone());
    let endpoints_hm: Arc<HashMap<u32, String>> = Arc::new(endpoint.clone().into_iter()
                .enumerate()
                .map(|(i, endpoint)| (i as u32, endpoint))
                .collect());
    // let cm = SyncArbiter::start(2, move || ChannelManager::new(current_node, endpoints_hm.clone(), Duration::from_millis(300)));
    let cm = ChannelManager::new(current_node, endpoints_hm.clone(), Duration::from_millis(300)).start();
    

    let node = Node { root_actor: root_actor_pool, channel_manager: Arc::new(cm.clone()) };


    let channel_manager = Data::new(cm);

    let endpoint_clone = Data::new(endpoint.clone());

    let http_server = HttpServer::new(move || App::new()
    .app_data(Data::clone(&root_actor))
    .app_data(Data::clone(&channel_manager))
    .app_data(Data::new(current_node))
    .app_data(Data::clone(&endpoint_clone))
    .app_data(Data::new(Client::default()))
    .service(index)
    .service(put)
    .service(get))
    .bind(("0.0.0.0", port)).unwrap()
    .run();

    let reflection_service = tonic_reflection::server::Builder::configure()
    .register_encoded_file_descriptor_set(rs::rs::FILE_DESCRIPTOR_SET)
    .build_v1()
    .unwrap();

    let grpc = Server::builder()
    .add_service(reflection_service)
    .add_service(RsServer::new(node))
    .serve(format!("0.0.0.0:{}", port+80).parse().unwrap());
    
    join!(grpc, http_server);

    Ok(())
}

