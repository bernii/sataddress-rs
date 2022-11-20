use anyhow::{bail, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;
use warp::hyper::{self, Body, Client, Method, Request, Uri};

use crate::LNbitsConfig;

pub async fn provision_backend(
    conf: &LNbitsConfig,
    username: &str,
    domain: &str,
    pub_key: &str,
) -> Result<(String, String, String), anyhow::Error> {
    let wallet_name = "scrub_wallet";
    let username = format!("{}@{}", username, domain);

    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("{}usermanager/api/v1/users", &conf.url))
        .header("X-Api-Key", &conf.api_key)
        .header("content-type", "application/json");

    let body = json!({
        "admin_id": &conf.admin_id,
        "wallet_name": wallet_name,
        "user_name": username,
    });
    let req = req.body(Body::from(body.to_string()))?;
    let client = Client::new();
    let resp = client.request(req).await?;

    // extract user_wallet_id, user_id, user_admin_key
    let bytes = hyper::body::to_bytes(resp).await?;
    let data = String::from_utf8(bytes.to_vec())?;
    let data: Value = serde_json::from_str(&data)?;

    let user_id = data["id"].as_str().unwrap();
    let user_admin_key = data["wallets"][0]["adminkey"].as_str().unwrap();
    let user_wallet_id = data["wallets"][0]["id"].as_str().unwrap();

    info!(
        "Created new lnbits user user_id:{} admin:{} wallet:{}",
        user_id, user_admin_key, user_wallet_id
    );

    // enable scrub extension for the user
    let mut url = Url::parse(&format!("{}usermanager/api/v1/extensions", &conf.url))?;
    url.query_pairs_mut()
        .append_pair("extension", "scrub")
        .append_pair("userid", user_id)
        .append_pair("active", "true")
        .finish();

    let req = Request::builder()
        .method(Method::POST)
        .uri(url.as_str())
        .header("X-Api-Key", &conf.api_key)
        .header("content-type", "application/json");

    info!("FULL URL is: {}", url.as_str());

    let req = req.body(Body::from(""))?; //req.body(Body::from(body.to_string()))?;
    let client = Client::new();
    let resp = client.request(req).await?;

    info!(
        "scrub enabled for wallet of user {}, status: {}",
        user_id,
        resp.status()
    );
    // create the scrub

    let api = ScrubApi {
        host: conf.url.clone(),
        api_key: user_admin_key.to_string(),
        wallet_id: Some(user_wallet_id.to_string()),
    };
    api.create(&format!("Payment via {}", &domain), pub_key)
        .await?;

    Ok((
        user_id.to_string(),
        user_admin_key.to_string(),
        user_wallet_id.to_string(),
    ))
}

#[derive(Debug, Serialize, Deserialize)]
struct ScrubApiEntry {
    id: String,
    description: String,
    wallet: String,
    payoraddress: String,
}

pub async fn update_entry(
    conf: &LNbitsConfig,
    api_key: &str,
    pub_key: Option<&str>,
    description: Option<&str>,
) -> Result<(), anyhow::Error> {
    if pub_key.is_none() && description.is_none() {
        bail!("Please provide new pub_key or description");
    }
    let mut api = ScrubApi {
        host: conf.url.clone(),
        api_key: api_key.to_string(),
        wallet_id: None,
    };
    let scrubs = api.list().await?;

    if scrubs.len() > 1 {
        bail!("Has multiple scrubs defined!");
    }
    let scrub = scrubs.get(0).unwrap();
    api.wallet_id = Some(scrub.wallet.to_string());

    let pub_key = pub_key.or(Some(&scrub.payoraddress)).unwrap();
    let description = description.or(Some(&scrub.description)).unwrap();
    api.update(&scrub.id, pub_key, description).await?;
    Ok(())
}

#[derive(Debug, Default)]
struct ScrubApi {
    host: Uri,
    api_key: String,
    wallet_id: Option<String>,
}

impl ScrubApi {
    async fn list(&self) -> Result<Vec<ScrubApiEntry>> {
        let client = Client::new();
        let req = Request::builder()
            .method(Method::GET)
            .uri(format!("{}scrub/api/v1/links", self.host))
            .header("X-Api-Key", self.api_key.to_string())
            .header("content-type", "application/json");

        let req = req.body(Body::default())?;
        let resp = client.request(req).await?;

        info!("got scrubs status: {}", resp.status());
        let bytes = hyper::body::to_bytes(resp).await?;
        let data = String::from_utf8(bytes.to_vec())?;
        let ret: Vec<ScrubApiEntry> = serde_json::from_str(&data)?;
        debug!("Found folling scrubs! {:?}", data);
        Ok(ret)
    }
    async fn create(self, description: &str, pub_key: &str) -> Result<()> {
        let client = Client::new();
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("{}scrub/api/v1/links", self.host))
            .header("X-Api-Key", self.api_key)
            .header("content-type", "application/json");

        let body = json!({
            "wallet": self.wallet_id,
            "description": description,
            "payoraddress": pub_key,
        });
        let req = req.body(Body::from(body.to_string()))?;
        let resp = client.request(req).await?;

        info!("scrub created status: {}", resp.status());
        Ok(())
    }
    async fn update(&self, scrub_id: &str, pub_key: &str, description: &str) -> Result<()> {
        debug!(
            "Updating scrub {:?} {} {} {}",
            self, scrub_id, pub_key, description
        );
        let client = Client::new();
        let req = Request::builder()
            .method(Method::PUT)
            .uri(format!("{}scrub/api/v1/links/{}", self.host, scrub_id))
            .header("X-Api-Key", self.api_key.to_string())
            .header("content-type", "application/json");

        let body = json!({
            "wallet": self.wallet_id,
            "description": description,
            "payoraddress": pub_key,
        });
        let req = req.body(Body::from(body.to_string()))?;
        let resp = client.request(req).await?;

        info!("scrub updated status: {}", resp.status());
        Ok(())
    }
}
