/// general data manipulation api
use crate::{
    db::{
        models::{Params, Stats},
        Db,
    },
    with_clone,
};
use log::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use warp::{host::Authority, reject::Reject, Filter, Rejection};

use super::Config;
use std::convert::Infallible;

#[derive(Debug)]
struct AuthError;

impl Reject for AuthError {}

pub async fn authenticate(
    pin: String,
    config: Config,
    // _body: HashMap<String, String>,
) -> Result<(), Rejection> {
    info!("The pin header value is equal to {}", pin);
    // TODO: this needs to be finished so simple
    // mitm does not reveal the pin
    // current assumption is that those APIs
    // are accessed via localhost from cli
    if pin != format!("TODO-{}", config.pin_secret) {
        return Err(warp::reject::custom(AuthError));
    }
    Ok(())
}

pub async fn check_domain(
    db: Db,
    config: Config,
    username: String,
    host: Option<Authority>,
) -> Result<(crate::db::Db, Config, String, String), warp::Rejection> {
    // extract from host header and
    // validate if it's in config.domains
    let domain = match host {
        None => return Err(warp::reject()),
        Some(host) => host.host().to_owned(),
    };
    if !config.domains.contains(&domain) {
        return Err(warp::reject());
    }
    Ok((db, config, username, domain))
}

// construct api handlers
pub fn handlers(
    db: crate::db::Db,
    config: crate::Config,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let base = warp::path!("user")
        .and(with_clone(db.clone()))
        .and(with_clone(config));

    let add_user = base.clone().and(warp::post()).and_then(add_user);
    let edit_user = base.clone().and(warp::put()).and_then(edit_user);
    let delete_user = base.clone().and(warp::delete()).and_then(delete_user);
    let get_user = base.and(warp::get()).and_then(get_user);
    let get_stats = warp::path!("stats")
        .and(with_clone(db))
        .and(warp::get())
        .and_then(get_stats);

    add_user
        .or(edit_user)
        .or(delete_user)
        .or(get_user)
        .or(get_stats)
}

pub async fn add_user(_db: Db, _config: Config) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::Response::new(
        "message: add".to_string().into(),
    ))
}

pub async fn edit_user(_db: Db, _config: Config) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::Response::new(
        "message: edit".to_string().into(),
    ))
}

pub async fn delete_user(_db: Db, _config: Config) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::Response::new(
        "message: del".to_string().into(),
    ))
}

pub async fn get_user(_db: Db, _config: Config) -> Result<impl warp::Reply, Infallible> {
    Ok(warp::reply::Response::new(
        "message: get".to_string().into(),
    ))
}

pub async fn get_stats(db: Db) -> Result<impl warp::Reply, Infallible> {
    let (data, summary) = generate_stats(&db).unwrap();
    Ok(warp::reply::json(&json!({
        "data": data,
        "summary": summary
    })))
}

pub fn generate_stats(db: &Db) -> Result<(HashMap<String, Stats>, Value), anyhow::Error> {
    let mut data = HashMap::new();
    let mut summary: Value = json!(
        {"calls": 0, "edits": 0, "invoices": 0}
    );

    for r in db.iter() {
        let ivec = r?;
        let p: Params = rmp_serde::from_slice(&ivec.1)?;

        let calls = summary["calls"].as_u64().unwrap() as u16 + p.stats.calls.num;
        summary["calls"] = serde_json::Value::Number(calls.into());

        let edits = summary["edits"].as_u64().unwrap() as u16 + p.stats.edits.num;
        summary["edits"] = serde_json::Value::Number(edits.into());

        let invoices = summary["invoices"].as_u64().unwrap() as u16 + p.stats.invoices.num;
        summary["invoices"] = serde_json::Value::Number(invoices.into());

        data.insert(format!("{}@{}", p.name, p.domain), p.stats);
    }
    Ok((data, summary))
}
