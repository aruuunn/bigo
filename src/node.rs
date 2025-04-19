use std::sync::Arc;

use actix::Addr;
use log::info;
use tonic::{Request, Response, Status};
use crate::location_actor::{GetLocation, GetShard, PutShard};
use crate::root_actor::{GetAddr, RootActor};
use crate::rs::rs::{self, EnrichedLocationStats, GetShardRequest, GetShardResponse};
use crate::rs::rs::{RouteWriteRequest, RouteWriteResponse, WriteShardRequest, WriteShardResponse};


#[derive(Debug)]
pub struct Node {
    pub root_actor: Addr<RootActor>
}

#[tonic::async_trait]
impl rs::rs_server::Rs for Node {
    async fn route_write(&self, request: Request<RouteWriteRequest>) -> Result<Response<RouteWriteResponse>, Status> {

        todo!()
    }

    async fn write_shard_request(&self, request: Request<WriteShardRequest>) -> Result<Response<WriteShardResponse>, Status> {
        let data = request.into_inner();
        info!("Write shard request: {}", data.location_id);
        let addr =  self.root_actor.send(GetAddr(data.location_id)).await.unwrap().unwrap();
        addr.send(PutShard(data.shard)).await.unwrap()
            .map(|_| Response::new(WriteShardResponse {}))
            .map_err(|err| Status::internal(err.to_string()))
    }

    async fn get_shard_request(&self, request: Request<GetShardRequest>) -> Result<Response<GetShardResponse>, Status> {
        let data = request.into_inner();
        let addr =  self.root_actor.send(GetAddr(data.location_id)).await.unwrap().unwrap();
        let shard = addr.send(GetShard{}).await.unwrap().ok();
        let location_stats = addr.send(GetLocation{}).await.unwrap().ok();

        if shard.is_none() && location_stats.is_none() {
            return Err(Status::not_found("shard and location are none"));
        }

        return Ok(Response::new(GetShardResponse { shard: shard, location_stats: location_stats.map(|l| EnrichedLocationStats {
            id:l.id,seismic_activity:l.seismic_activity, temperature_c: l.temperature_c, radiation_level: l.radiation_level, modification_count: l.modification_count }) }))
    }
}
