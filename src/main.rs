use actix_web::error::InternalError;
use actix_web::Result;
use actix_web::{get, middleware::Logger, web, App, HttpResponse, HttpServer, Responder};
use env_logger::Env;
use flate2::write::GzEncoder;
use flate2::Compression;
use pprof::protos::Message;
use pprof::ProfilerGuardBuilder;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Deserialize)]
struct ProfileParams {
    seconds: Option<u64>,
}

#[get("/debug/pprof/profile")]
async fn pprof_profile(
    params: web::Query<ProfileParams>,
) -> Result<impl Responder, actix_web::Error> {
    let guard = ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .map_err(|e| {
            InternalError::from_response(
                "Failed to start profiling",
                HttpResponse::InternalServerError().body(format!("{:?}", e)),
            )
        })?;

    let duration = params.seconds.unwrap_or(30);
    sleep(Duration::from_secs(duration)).await;

    let profile = guard
        .report()
        .build()
        .map_err(|e| {
            InternalError::from_response(
                "Failed to build report",
                HttpResponse::InternalServerError().body(format!("{:?}", e)),
            )
        })?
        .pprof()
        .map_err(|e| {
            InternalError::from_response(
                "Failed to convert report to pprof",
                HttpResponse::InternalServerError().body(format!("{:?}", e)),
            )
        })?;

    let mut body = Vec::new();
    let mut encoder = GzEncoder::new(&mut body, Compression::default());

    profile.write_to_writer(&mut encoder).map_err(|e| {
        InternalError::from_response(
            "Failed to encode report",
            HttpResponse::InternalServerError().body(format!("{:?}", e)),
        )
    })?;

    encoder.finish().map_err(|e| {
        InternalError::from_response(
            "Failed to finish encoding report",
            HttpResponse::InternalServerError().body(format!("{:?}", e)),
        )
    })?;

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"profile.pb.gz\"",
        ))
        .body(body))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    HttpServer::new(|| App::new().wrap(Logger::default()).service(pprof_profile))
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
