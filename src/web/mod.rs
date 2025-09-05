use std::io;
use actix_web::{middleware, get, web, App, Error as AxError, HttpResponse, HttpServer, Error};
use actix_web::error::{ErrorInternalServerError, ErrorNotFound, ParseError};
use actix_web::http::{StatusCode};

use std::path::PathBuf;
use actix_web::http::header::{ContentDisposition, ContentType};
use anyhow::Result;

pub mod db;
use db::{Pool};
use crate::utils::open;
use crate::web::db::{execute, Queries};
use crate::web::db::DataType::Data;

#[get("/")]
async fn index() -> actix_web::Result<HttpResponse> {
    let p = String::from("assets/web/index.html");
    let x = open(PathBuf::from(p));
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(x))
}

#[get("/css/{css}")]
async fn css(path: web::Path<String>) -> actix_web::Result<HttpResponse, AxError> {
    let css = path.into_inner();

    let p = "assets/web/css/".to_owned() + &css;
    match web::block(|| std::fs::read(PathBuf::from(p))).await {
        Ok(css) => match css {
            Ok(css) => Ok(HttpResponse::build(StatusCode::OK)
                .content_type(ContentType(mime::TEXT_CSS_UTF_8))
                .body(css)),
            Err(e) => Err(ErrorNotFound(e)),
        },
        Err(e) => Err(ErrorInternalServerError(e)),
    }
}

#[get("/js/{js}")]
async fn js(path: web::Path<String>) -> actix_web::Result<HttpResponse, AxError> {
    let js = path.into_inner();

    let p = "assets/web/js/".to_owned() + &js;
    match web::block(|| std::fs::read(PathBuf::from(p))).await {
        Ok(js) => match js {
            Ok(js) => Ok(HttpResponse::build(StatusCode::OK)
                .content_type(ContentType(mime::APPLICATION_JAVASCRIPT_UTF_8))
                .body(js)),
            Err(e) => Err(ErrorNotFound(e)),
        },
        Err(e) => Err(ErrorInternalServerError(e)),
    }
}

#[get("/api/all")]
async fn voicemail_all(db: web::Data<Pool>) -> Result<HttpResponse, AxError> {
    match execute(&db, Queries::AllVoicemail).await {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(e) => {
            log::info!("list empty {e:?}");
            Ok(HttpResponse::Ok().json(""))
        }
    }
}

#[get("/api/del/{id}")]
async fn del_voicemail(db: web::Data<Pool>, path: web::Path<String>) -> Result<HttpResponse, AxError> {
    let id = path.into_inner().parse::<i64>().expect("delete voicemail");
    let result = execute(&db, Queries::DeleteVoicemail(id)).await?;
    Ok(HttpResponse::Ok().json(result))
}

#[get("/api/voice/{id}")]
async fn voice_data(db: web::Data<Pool>, path: web::Path<String>) -> Result<HttpResponse, AxError> {
    let id = path.into_inner().parse::<i64>().expect("get voice data");
    match execute(&db, Queries::VoiceData(id)).await?.first() {
        Some(Data { data }) => {
            let cd = ContentDisposition::attachment(format!("{}.au", id));
            Ok(HttpResponse::Ok().content_type("audio/basic")
                .append_header(cd)
                .body(data.clone()))
        }
        _ => Err(Error::from(ParseError::Incomplete)),
    }
}

pub async fn server(pool: Pool) -> io::Result<()> {
    log::info!("starting HTTP server at http://localhost:8080");

    // start HTTP server
    HttpServer::new(move || {
        App::new()
            // store db pool as Data object
            .app_data(web::Data::new(pool.clone()))
            .wrap(middleware::Logger::default())
            .service(index)
            .service(css)
            .service(js)
            .service(voicemail_all)
            .service(del_voicemail)
            .service(voice_data)
    })
        .bind(("127.0.0.1", 8080))?
        .workers(2)
        .run()
        .await
}
