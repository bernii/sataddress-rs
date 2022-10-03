use std::env;

use envconfig::Envconfig;
use sataddress::{api, db, handlers, with_clone, Config};
use warp::Filter;

use log::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=todos=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "debug");
    }
    pretty_env_logger::init();

    let config = Config::init_from_env().unwrap();
    debug!("Running with the following config {:?}", config);

    let db = db::Db::init().unwrap();

    let base_dir = format!("{}/", env!("CARGO_MANIFEST_DIR"));

    // GET /
    let index = warp::path::end()
        .and(with_clone(config.clone()))
        .and_then(handlers::index);

    // GET /static/*
    let statics = warp::path("static").and(warp::fs::dir(format!("{}assets/", base_dir)));

    // basic injection of config and db connector to handlers
    let base = warp::any()
        .and(with_clone(db.clone()))
        .and(with_clone(config.clone()));

    // handle LNURL calls
    let ln_url = base
        .clone()
        .and(warp::path!(".well-known" / "lnurlp" / String))
        // .and(warp::query::<LnURLparams>())
        .and(warp::host::optional())
        .and_then(api::check_domain)
        .untuple_one()
        .and(warp::query::<HashMap<String, String>>())
        .and_then(handlers::lnurl);

    // wizard add/update of an alias
    let grab = base
        .clone()
        .and(warp::path("grab"))
        .and(warp::body::aggregate())
        .and_then(handlers::grab);

    // basic REST API to manage entries in the DB
    let api = warp::path!("api" / "v1" / ..)
        .and(warp::header::<String>("X-PIN"))
        .and(with_clone(config.clone()))
        // .and(warp::body::json())
        .and_then(api::authenticate)
        .untuple_one()
        .and(api::handlers(db.clone(), config.clone()));

    let routes = warp::any().and(
        index
            .or(statics)
            .or(ln_url)
            .or(grab)
            .or(api)
            .recover(handlers::handle_rejection),
    );

    info!("Starting server...");
    warp::serve(routes).run((config.host, config.port)).await;
}
