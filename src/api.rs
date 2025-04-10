use std::sync::mpsc::channel;
use std::time::Duration;
use actix::{Actor, Addr};
use actix_web::{put, web, App, HttpResponse, HttpServer};
use actix_web::{get, HttpRequest, Responder};
use actix_web::dev::Path;
use actix_web::test::status_service;
use actix_web::web::{Data, Json};
use tonic::IntoRequest;
use crate::conn_manager::ChannelManager;
use crate::dto::{LocationStats};
use crate::location_actor::{GetLocation, PutLocation};
use crate::root_actor::{GetAddr, RootActor};


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
async fn get(id: web::Path<(String)>, root_actor: Data<Addr<RootActor>>) -> impl Responder {
    let (location_id) = id.into_inner();
    let addr =  root_actor.send(GetAddr(location_id)).await.unwrap().unwrap();
    let location = addr.send(GetLocation).await.unwrap().unwrap();
    HttpResponse::Ok().json(location)
}


pub async fn bootstrap(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let root_actor = Data::new(RootActor::new().start());
    let channel_manager = Data::new(ChannelManager::new(Duration::from_millis(300)).start());
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