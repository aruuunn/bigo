use actix::{Actor, Addr};
use actix_web::{web, App, HttpServer};
use actix_web::{get, HttpRequest, Responder};
use actix_web::dev::Path;
use actix_web::web::{Data, Json};
use crate::dto::LocationDTO;
use crate::root_actor::{GetAddr, RootActor};


#[get("/ping")]
async fn index(_req: HttpRequest) -> impl Responder {
    "pong"
}

#[get("/put")]
async fn put(thing: Json<LocationDTO>, id: Path<String>, root_actor: Data<Addr<RootActor>>) -> impl Responder {
   let addr =  root_actor.send(GetAddr(id.get())).await?;

}

pub async fn bootstrap(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let root_actor_addr = RootActor::new().start();

    HttpServer::new(|| App::new()
        .app_data(Data::new(root_actor_addr))
        .service(index)
        .route("/", web::put().to(put)))
        .bind(("0.0.0.0", port))?
        .run()
        .await?;

    Ok(())
}