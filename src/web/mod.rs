use actix_web::cookie::ParseError::EmptyName;
use actix_web::error::{ErrorInternalServerError, ErrorNotFound, ParseError};
use actix_web::http::StatusCode;
use actix_web::http::header::{ContentDisposition, ContentType};
use actix_web::{App, Error as AcError, Error, HttpResponse, HttpServer, Result as AcResult, get, middleware, web, put};
use anyhow::Result;
use std::io;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

pub mod db;
use crate::utils::{open, trim_null_bytes};
use crate::web::db::DataType::Data;
use crate::web::db::{Queries, execute};
use db::Pool;

#[get("/")]
async fn index() -> AcResult<HttpResponse> {
    let p = String::from("assets/web/index.html");
    let x = open(PathBuf::from(p));
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(x))
}

#[get("/api/all")]
async fn voicemail_all(db: web::Data<Pool>) -> Result<HttpResponse, AcError> {
    match execute(&db, Queries::AllVoicemail).await {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(e) => {
            log::info!("list empty {e:?}");
            Ok(HttpResponse::Ok().json(""))
        }
    }
}

#[get("/{path}/{file}")]
async fn assets(assets: web::Path<(String, String)>) -> Result<HttpResponse, AcError> {
    let (path, file) = assets.into_inner();

    let p = format!("assets/web/{}/{}", path, file);
    match web::block(|| std::fs::read(PathBuf::from(p))).await {
        Ok(d) => match path {
            s if s == "js" => Ok(HttpResponse::build(StatusCode::OK)
                .content_type(ContentType(mime::APPLICATION_JAVASCRIPT_UTF_8))
                .body(d?)),
            s if s == "css" => Ok(HttpResponse::build(StatusCode::OK)
                .content_type(ContentType(mime::TEXT_CSS_UTF_8))
                .body(d?)),
            s if s == "img" => Ok(HttpResponse::build(StatusCode::OK)
                .content_type(ContentType(mime::IMAGE_SVG))
                .body(d?)),
            _ => Err(ErrorNotFound(EmptyName)),
        },
        Err(e) => Err(ErrorInternalServerError(e)),
    }
}

#[get("/api/del/{id}")]
async fn del_voicemail(
    db: web::Data<Pool>,
    path: web::Path<String>,
) -> Result<HttpResponse, AcError> {
    let id = path.into_inner().parse::<i64>().expect("delete voicemail");
    let result = execute(&db, Queries::DeleteVoicemail(id)).await?;
    Ok(HttpResponse::Ok().json(result))
}


#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub tel: String,
    pub name: String,
}

#[put("/api/mod")]
async fn modify_caller (
    db: web::Data<Pool>,
    item: web::Json<User>,
) -> Result<HttpResponse, AcError> {
    log::info!("{item:?}");
    let tel = item.tel.to_owned();

    let result = match item.name.as_str().trim() {
        "" => execute(&db, Queries::DeleteContacts(tel)).await?,
        name => execute(&db, Queries::AddContacts(tel, name.to_owned())).await?,
    };
    Ok(HttpResponse::Ok().json(result))
}

#[get("/api/voice/{id}")]
async fn voice_data(db: web::Data<Pool>, path: web::Path<String>) -> Result<HttpResponse, AcError> {
    let id = path.into_inner().parse::<i64>().expect("get voice data");
    match execute(&db, Queries::VoiceData(id)).await?.first() {
        Some(Data { data }) => {
            let d = trim_null_bytes(data);
            let cd = ContentDisposition::attachment(format!("{}.au", id));
            Ok(HttpResponse::Ok()
                .content_type("audio/basic")
                .append_header(cd)
                .body(d.to_vec()))
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
            .service(voicemail_all)
            .service(del_voicemail)
            .service(voice_data)
            .service(modify_caller)
            .service(assets)
    })
    .bind(("0.0.0.0", 8080))?
    .workers(2)
    .run()
    .await
}
