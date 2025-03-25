// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
// use actix::prelude::*;
// use actix_async_handler::async_handler;
// use tokio::sync::Mutex;
// use tonic::client::GrpcService;
// use tonic::transport::{Channel, Endpoint, Uri};
//
// use actix::prelude::*;
//
// struct ConnManager {
//     addr: String,
//     channel: Channel,
// }
//
// impl Actor for ConnManager {
//     type Context = Context<Self>;
// }
//
// use actix::prelude::*;
//
// #[derive(Message)]
// #[rtype(result = "Channel")]
// struct GetChannel(Channel);
//
// #[derive(Message)]
// #[rtype(result = "()")]
// struct ResetConnection;
//
// impl Handler<GetChannel> for ConnManager {
//     type Result = Channel;
//
//     fn handle(&mut self, _: GetChannel, _ctx: &mut Context<Self>) -> Self::Result {
//         self.channel.clone()
//     }
// }
//
// impl Handler<ResetConnection> for ConnManager {
//     type Result = ();
//
//     fn handle(&mut self, _: GetChannel, _ctx: &mut Context<Self>) -> Self::Result {
//         self.channel = Endpoint::from_static(&self.addr).connect_lazy();
//     }
// }