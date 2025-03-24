use tonic::{Request, Response, Status};
use crate::rs::rs;
use crate::rs::rs::{RouteWriteRequest, RouteWriteResponse};


#[derive(Debug, Default)]
pub struct Node {}

#[tonic::async_trait]
impl rs::rs_server::Rs for Node {
    async fn route_write(&self, request: Request<RouteWriteRequest>) -> Result<Response<RouteWriteResponse>, Status> {

        todo!()
    }
}
