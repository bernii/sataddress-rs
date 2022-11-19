use serde::{Deserialize, Serialize};

/// Defines a structure that we use to generate
/// JSON response for LN URL
#[derive(Deserialize, Serialize, Default)]
pub struct SuccessAction {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub message: Option<String>,
}

/// Defines a structure that we use to generate
/// JSON response for LNURL Payment
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LNURLPayValues {
    #[serde(flatten)]
    pub lnurl_response: LNURLResponse,
    pub success_action: SuccessAction,
    pub pr: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub disposable: Option<bool>,
}

/// Defines a structure that we use to generate
/// JSON response for LNURL Response
#[derive(Deserialize, Serialize)]
pub struct LNURLResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub reason: Option<String>,
}

/// Defines LNURL payment parameters
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LNURLPayParams {
    #[serde(flatten)]
    pub lnurl_response: LNURLResponse,
    pub callback: String,
    pub tag: String,
    pub max_sendable: u64,
    pub min_sendable: u64,
    pub metadata: String,
    pub comment_allowed: u8,
}

/// BTC banner shown in wallets payment modals
const BTC_LN_IMG: &[u8] = include_bytes!("../assets/inv_banner.png");

/// Invoice generation and interaction logic
pub mod invoice {
    use std::time::Duration;

    use anyhow::bail;
    use log::debug;
    use serde_json::{json, Value};
    use tokio::{
        io::{AsyncRead, AsyncWrite},
        time::timeout,
    };

    use warp::hyper::{self, service::Service, Body, Client, Method, Request, Uri};

    use crate::db::models::{self, InvoiceAPI};
    use base64;
    use hyper_tls::{HttpsConnecting, HttpsConnector, MaybeHttpsStream};

    use hyper_socks2::SocksConnector;
    use hyper_tls::native_tls;
    use sha2::{Digest, Sha256};

    use super::BTC_LN_IMG;

    /// Used to generate descriptions and information
    /// about the payments
    pub struct Metadata {
        name: String,
        domain: String,
    }

    impl Metadata {
        // Recipient of the payment
        fn for_whom(&self) -> String {
            format!("{}@{}", self.name, self.domain)
        }
        // Description of the payment - often used as memo
        fn get_text(&self) -> String {
            format!("Satoshis for {}.", &self.for_whom())
        }
    }

    impl From<models::Params> for Metadata {
        fn from(params: models::Params) -> Metadata {
            Self {
                name: params.name,
                domain: params.domain,
            }
        }
    }

    impl From<&Metadata> for serde_json::Value {
        fn from(m: &Metadata) -> Self {
            serde_json::json!([
                ["text/identifier", m.for_whom()],
                ["text/plain", m.get_text()],
                ["image/png;base64", base64::encode(BTC_LN_IMG)],
            ])
        }
    }

    impl ToString for Metadata {
        fn to_string(&self) -> String {
            let json: serde_json::Value = self.into();
            json.to_string()
        }
    }

    /// Connector that can handle both regular HTTPS
    /// connection and SOCKS-proxied connection via
    /// configured SOCKS proxy.
    #[derive(Clone)]
    enum MaybeProxiedConnector<T> {
        Https(HttpsConnector<T>),
        Proxy(HttpsConnector<SocksConnector<T>>),
    }

    type BoxError = Box<dyn std::error::Error + Send + Sync>;

    impl<T> Service<Uri> for MaybeProxiedConnector<T>
    where
        T: Service<Uri> + Clone + Send + 'static,
        T::Response: AsyncRead + AsyncWrite + Send + Unpin,
        T::Error: Into<BoxError>,
        T::Future: Send,
    {
        type Response = MaybeHttpsStream<T::Response>;
        type Error = BoxError;
        type Future = HttpsConnecting<T::Response>;

        fn poll_ready(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            match self {
                MaybeProxiedConnector::Https(c) => c.poll_ready(cx),
                MaybeProxiedConnector::Proxy(c) => c.poll_ready(cx),
            }
        }

        fn call(&mut self, req: Uri) -> Self::Future {
            match self {
                MaybeProxiedConnector::Https(c) => c.call(req),
                MaybeProxiedConnector::Proxy(c) => c.call(req),
            }
        }
    }

    /// Connects to defined IncoiceAPI defined in Params in
    /// order to create an invoice based on the input data.
    pub async fn make_invoice(
        params: &models::Params,
        ln_host: &Uri,
        msat: u64,
        tor_proxy: Uri,
        memo: Option<String>,
    ) -> Result<Value, anyhow::Error> {
        // enforce https
        let mut http = hyper::client::HttpConnector::new();
        http.enforce_http(false);

        // accept self-signed certs
        // useful when dealing with self-hosted wallets
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let connector = tls;

        let https = match params.invoice_api.is_tor() {
            false => MaybeProxiedConnector::Https(HttpsConnector::from((http, connector.into()))),
            true => {
                let proxy = SocksConnector {
                    proxy_addr: tor_proxy, // scheme is required by HttpConnector
                    auth: None,
                    connector: http,
                };
                MaybeProxiedConnector::Proxy(HttpsConnector::from((proxy, connector.into())))
            }
        };

        let client = Client::builder().build::<_, hyper::Body>(https);

        let metadata = Metadata::from(params.clone());
        let metadata_sha = Sha256::new().chain_update(metadata.to_string()).finalize();
        let req: warp::http::request::Builder;
        let mut body: Value;

        match params.invoice_api.clone() {
            InvoiceAPI::Lnd(p) => {
                body = json!({
                    "value_msat": msat,
                    // "out": false,
                    // again, either memo (to just put invoice in wallet) or desc_hash if actual transiation
                    // "memo": metadata.get_text(),
                    // "description_hash": base64::encode(metadata_sha),
                });

                body["memo"] = match memo {
                    Some(memo) => serde_json::Value::String(memo),
                    None => serde_json::Value::String(metadata.get_text()),
                };
                body["description_hash"] = serde_json::Value::String(base64::encode(metadata_sha));

                let mut macaroon = p.macaroon.to_string();
                // macaroon needs to be a hex string
                // convert from base64 as that's how it's usually delivered
                if let Ok(decoded) = base64::decode(&macaroon) {
                    macaroon = hex::encode(decoded);
                }

                req = Request::builder()
                    .method(Method::POST)
                    .uri(format!("{}/v1/invoices", p.host))
                    .header("Grpc-Metadata-macaroon", macaroon)
                    .header("content-type", "application/json");
            }
            InvoiceAPI::LNBits(p) => {
                body = json!({
                    "amount": msat / 1000,
                    "out": false,
                    // "description": "satoshis incoming from SatAddress",
                    // "comment": "this is a comment",
                    // "memo": "this is a memo",
                    // for lnbits 0.8.0 support (umbrel)
                    // you need description_hash since unhashed_desc is not supported yet
                    // but then it stops working with latest ;) huh
                    // "description_hash": hex::encode(metadata_sha),
                    // "unhashed_description": hex::encode(metadata.to_string()),
                });

                // memo is ignored, see code links above
                match memo {
                    Some(memo) => {
                        body["memo"] = serde_json::Value::String(memo);
                    }
                    None => {
                        // TODO PR for LNBits
                        // https://github.com/lnbits/lnbits/blob/4ad3c841528de3efafefe48f667e6800eb7074e3/lnbits/core/services.py#L65
                        //
                        body["unhashed_description"] =
                            serde_json::Value::String(hex::encode(metadata.to_string()));
                    }
                }

                body["unhashed_description"] =
                    serde_json::Value::String(hex::encode(metadata.to_string()));

                debug!(
                    "Sending body {:?} to {:?} with key {:?}",
                    body, p.host, p.key
                );

                req = Request::builder()
                    .method(Method::POST)
                    .uri(format!("{}/api/v1/payments", p.host))
                    .header("X-Api-Key", p.key)
                    .header("content-type", "application/json");
            }
            InvoiceAPI::Keysend(p) => {
                // reject payments lower than 3 sats
                // as those probably won't cover payment fees
                // and transaction will get stuck :-(
                if msat < 3000 {
                    bail!("less than 3sats might not cover routing fees")
                }

                body = json!({
                    "amount": msat / 1000,
                    "out": false,
                });

                // memo is ignored, see code links above
                match memo {
                    Some(memo) => {
                        body["memo"] = serde_json::Value::String(memo);
                    }
                    None => {
                        body["unhashed_description"] =
                            serde_json::Value::String(hex::encode(metadata.to_string()));
                    }
                }

                body["unhashed_description"] =
                    serde_json::Value::String(hex::encode(metadata.to_string()));

                req = Request::builder()
                    .method(Method::POST)
                    .uri(format!("{}api/v1/payments", ln_host))
                    .header("X-Api-Key", p.admin_key.unwrap())
                    .header("content-type", "application/json");
            }
        }

        let req = req.body(Body::from(body.to_string()))?;
        let future = client.request(req);
        let resp = match timeout(Duration::from_secs(180), future).await {
            Ok(r) => r?,
            Err(_e) => bail!("Connection timeout error"),
        };

        let status = resp.status().as_u16();

        let bytes = hyper::body::to_bytes(resp).await?;
        let mut data = String::from_utf8(bytes.to_vec())?;

        if status >= 300 {
            data.truncate(300);
            bail!("Call to lnd failed ({}): {}", status, data)
        }

        let v: Value = match serde_json::from_str(&data) {
            Ok(json) => json,
            Err(e) => {
                data.truncate(500);
                debug!(
                    "Unable to parse json response the LN Node err: {:?}, data: {:?}",
                    e, data
                );
                bail!("Unable to parse json response from the LN Node");
            }
        };

        debug!(
            "Invoice generated [{:?}] for {} msat, inv: {}",
            params.invoice_api,
            msat,
            v["payment_request"].clone()
        );

        Ok(v["payment_request"].clone())
    }

    #[cfg(test)]
    mod tests {
        use serde_json::{json, Value};
        use warp::hyper::Uri;
        use wiremock::{
            http::HeaderName,
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        use super::{make_invoice, Metadata};
        use crate::db::models::{InvoiceAPI, Params};

        #[test]
        fn metadata_from_params() {
            let name = "my-username".to_string();
            let domain = "some-domain.com".to_string();
            let params = Params {
                name: name.clone(),
                domain: domain.clone(),
                ..Default::default()
            };
            let metadata: Metadata = params.into();
            assert_eq!(metadata.name, name);
            assert_eq!(metadata.domain, domain);
        }

        #[test]
        fn metadata_forms() {
            let name = "aname".to_string();
            let domain = "a-domain.com".to_string();
            let metadata = Metadata {
                name: name.clone(),
                domain: domain.clone(),
            };
            assert!(metadata.for_whom().contains(&name));
            assert!(metadata.for_whom().contains(&domain));
            assert!(metadata.get_text().contains("Satoshis for"));
            let s_meta = metadata.to_string();

            assert!(s_meta.contains("identifier"));
            assert!(s_meta.contains("plain"));
            assert!(s_meta.contains("png;base64"));
        }

        async fn prepare_server_mock() -> MockServer {
            let mock_server = MockServer::start().await;
            let resp = ResponseTemplate::new(200).set_body_json(json!({
                "payment_request": "abc-payment",
            }));
            Mock::given(method("POST"))
                .and(path("/v1/invoices"))
                .respond_with(resp)
                .mount(&mock_server)
                .await;
            mock_server
        }

        #[tokio::test]
        async fn make_invoice_calls_api() {
            let mock_server = prepare_server_mock().await;

            let mut params = Params::default();
            if let InvoiceAPI::Lnd(ref mut p) = params.invoice_api {
                p.host = mock_server.uri();
            }
            // invoke the method
            let result = make_invoice(
                &params,
                &"http://127.0.0.0.1".parse::<Uri>().unwrap(),
                1000,
                "http://127.0.0.0.1".parse::<Uri>().unwrap(),
                Some("memo".to_string()),
            )
            .await
            .unwrap();
            // mock checks
            mock_server.verify().await;
            let rcv_req = mock_server.received_requests().await.unwrap();
            assert_eq!(rcv_req.len(), 1);
            let req = rcv_req.first().unwrap();
            let rcv_body = req.body_json::<Value>().unwrap();
            assert_eq!(rcv_body["value_msat"].as_i64().unwrap(), 1000);
            assert!(rcv_body["memo"].is_string());
            assert!(rcv_body["description_hash"].is_string());
            assert!(req
                .headers
                .contains_key(&HeaderName::from("grpc-metadata-macaroon")));
            // actual response check
            assert_eq!(result, "abc-payment");
        }
    }
}
