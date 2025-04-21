use std::sync::Arc;

use actix::Addr;
use log::{error, info};
use tonic::{Request, Response, Status};
use crate::conn_manager::ChannelManager;
use crate::constants::ROOT_ACTOR_POOL_SIZE;
use crate::dto::{ExtendedLocationStats, LocationStats};
use crate::location_actor::{GetLocation, GetShard, PutLocation, PutShard};
use crate::root_actor::{self, GetAddr, RootActor};
use crate::rs::rs::{self, EnrichedLocationStats, GetShardRequest, GetShardResponse};
use crate::rs::rs::{RouteWriteRequest, RouteWriteResponse, WriteShardRequest, WriteShardResponse};
use crate::util::get_owner_node_id;


pub struct Node {
    pub root_actor: Vec<Addr<RootActor>>,
    pub channel_manager: Arc<Addr<ChannelManager>>
}

#[tonic::async_trait]
impl rs::rs_server::Rs for Node {
    async fn route_write(&self, request: Request<RouteWriteRequest>) -> Result<Response<RouteWriteResponse>, Status> {
        let data = request.into_inner();
        let location_id = data.location_id.clone();
        let root_actor = self.root_actor.get((get_owner_node_id(data.location_id.clone()) % ROOT_ACTOR_POOL_SIZE) as usize).unwrap();
        let addr =  root_actor.send(GetAddr(data.location_id.clone())).await.unwrap().unwrap();

        if let Err(err) = addr.send(PutLocation(ExtendedLocationStats::from_basic(data.location_id.clone(), LocationStats::from(data)), self.channel_manager.clone())).await {
            error!("Failed to put location stats: {} {}", err, location_id);
            return Err(Status::internal(err.to_string()));
        }

   Ok(Response::new(RouteWriteResponse {}))
}

    async fn write_shard_request(&self, request: Request<WriteShardRequest>) -> Result<Response<WriteShardResponse>, Status> {
        let data = request.into_inner();
        info!("Write shard request: {}", data.location_id);
        let root_actor = self.root_actor.get((get_owner_node_id(data.location_id.clone()) % ROOT_ACTOR_POOL_SIZE) as usize).unwrap();
        let addr =  root_actor.send(GetAddr(data.location_id)).await.unwrap().unwrap();
        addr.send(PutShard(data.shard)).await.unwrap()
            .map(|_| Response::new(WriteShardResponse {}))
            .map_err(|err| Status::internal(err.to_string()))
    }

    async fn get_shard_request(&self, request: Request<GetShardRequest>) -> Result<Response<GetShardResponse>, Status> {
        let data = request.into_inner();
        let root_actor = self.root_actor.get((get_owner_node_id(data.location_id.clone()) % ROOT_ACTOR_POOL_SIZE) as usize).unwrap();
        let addr =  root_actor.send(GetAddr(data.location_id.clone())).await.unwrap().unwrap();
        let shard = addr.send(GetShard(data.location_id)).await.unwrap().ok();
        let location_stats = addr.send(GetLocation{}).await.unwrap().ok();

        if shard.is_none() && location_stats.is_none() {
            return Err(Status::not_found("shard and location are none"));
        }

        return Ok(Response::new(GetShardResponse { shard: shard, location_stats: location_stats.map(|l| EnrichedLocationStats {
            id:l.id,seismic_activity:l.seismic_activity, temperature_c: l.temperature_c, radiation_level: l.radiation_level, modification_count: l.modification_count }) }))
    }
}
