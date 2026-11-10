use crate::routes;
use actix_web::dev::Server;
use actix_web::{App, HttpServer, web};
use actix_web::middleware::Logger;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub fn run(listener: TcpListener, pool: PgPool) -> Result<Server, std::io::Error> {
    // Wrap the pool in web::Data (an Arc pointer) so it can be cheaply cloned
    // across all of Actic-web's worker threads
    let pool = web::Data::new(pool);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .app_data(pool.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
