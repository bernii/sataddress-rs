use crate::{
    db::{
        defaults,
        models::{InvoiceAPI, Params},
        Db,
    },
    keysend,
    ln::{
        invoice::{make_invoice, Metadata},
        LNURLPayParams, LNURLPayValues, LNURLResponse, SuccessAction,
    },
};

use log::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

use validator::{Validate, ValidateArgs, ValidationError, ValidationErrors, ValidationErrorsKind};
use warp::{
    hyper::StatusCode,
    reject::{self, Reject},
    Buf, Rejection, Reply,
};

use super::Config;
use std::{collections::HashMap, convert::Infallible, error::Error as StdError};
use strum::IntoEnumIterator;

use percent_encoding::percent_decode_str;

use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemaplte<'a> {
    domains: &'a Vec<String>,
    site_name: &'a str,
    site_sub_name: &'a str,
}

/// Renders the main (and only) web papge
pub async fn index(config: Config) -> Result<impl warp::Reply, warp::Rejection> {
    let i_template = IndexTemaplte {
        domains: &config.domains.into(),
        site_name: &config.site_name,
        site_sub_name: &config.site_sub_name,
    };
    let body = i_template.render().unwrap();
    Ok(warp::reply::html(body))
}

#[derive(Debug)]
struct LnUrlError(String);
impl Reject for LnUrlError {}

/// Handles LN URL requests. There are two types of requests:
/// 1. request specifying `amount`
/// If such request is recieved, we contact the wallet node
/// via the API in order to create an invoice and return
/// in to the caller.
/// 2. requess not specifying `amount`
/// If such request is recieved we generate a LNURLPayParams
/// response which is then used in the customer wallet
/// to render the payment form.
pub async fn lnurl(
    db: Db,
    config: Config,
    username: String,
    domain: String,
    query: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let username = percent_decode_str(&username)
        .decode_utf8()
        .map_err(|_| warp::reject())?
        .to_string();

    info!(
        "Got LN URL request - username: {}, domain: {}",
        username, domain,
    );

    debug!("LN URL request data {}@{} {:?}", username, domain, query,);

    let mut params = db
        .get(&username, &domain)
        .map_err(|_| warp::reject())?
        .ok_or_else(warp::reject)?;

    match query.get("amount") {
        Some(msat) => {
            let msat = msat.parse::<u64>().map_err(|_| warp::reject())?;

            let memo = match query.get("comment") {
                Some(s) if !s.is_empty() => Some(s.to_owned()),
                _ => None,
            };
            if let (Some(memo), InvoiceAPI::Keysend(params)) = (&memo, &params.invoice_api) {
                // payment for keysend
                // update the scrub so that it matches the comment
                // in the request
                keysend::update_entry(
                    &config.lnbits,
                    params.admin_key.as_ref().unwrap(),
                    None,
                    Some(memo),
                ).await
                .map_err(|e| Error::Val(format!("Problem updating keysend data: {}", e)))?;
            }
            let bolt11 = make_invoice(
                &params,
                &config.lnbits.url,
                msat,
                config.tor_proxy_url,
                memo,
            )
            .await
            .map_err(|e| reject::custom(LnUrlError(e.to_string())))?;

            let success_action = SuccessAction {
                tag: "message".to_string(),
                message: Some("Payment received!".to_string()),
                ..Default::default()
            };

            params.stats.invoices.inc();
            db.update(&params).map_err(|_| warp::reject())?;

            let resp = LNURLPayValues {
                lnurl_response: LNURLResponse {
                    status: Some("OK".to_string()),
                    reason: None,
                },
                pr: bolt11,
                disposable: Some(false),
                success_action,
            };

            Ok(warp::reply::json(&resp))
        }
        None => {
            // no amount provided, different payload

            // TODO: support webhook comments
            let min_sendable = params.min_sendable.unwrap_or(defaults::MIN_SENDABLE);
            let max_sendable = params.max_sendable.unwrap_or(defaults::MAX_SENDABLE);

            params.stats.calls.inc();
            db.update(&params).map_err(|_| warp::reject())?;

            Ok(warp::reply::json(&LNURLPayParams {
                lnurl_response: LNURLResponse {
                    status: Some("OK".to_string()),
                    reason: None,
                },
                callback: format!("https://{}/.well-known/lnurlp/{}", domain, username),
                min_sendable,
                max_sendable,
                metadata: Metadata::from(params.clone()).to_string(),
                comment_allowed: params.invoice_api.get_comment_len(),
                tag: "payRequest".to_owned(),
            }))
        }
    }
}

/// Format of the POST request used to reserve/claim addresses
/// in the system and to mofidy entries (PIN required)
#[derive(Deserialize, Debug, Validate)]
struct AliasPostData {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(custom(function = "validate_domain", arg = "&'v_a Config"))]
    pub domain: String,
    #[validate(custom(function = "validate_backend", arg = "&'v_a Config"))]
    pub backend: String,
    pub pin: Option<String>,
    pub backend_data: Option<InvoiceAPI>,
}

impl From<AliasPostData> for Params {
    fn from(data: AliasPostData) -> Self {
        let min_sendable = match data.backend_data.as_ref().unwrap() {
            // less than 3k msats might mean routing problems
            InvoiceAPI::Keysend(_) => Some(3000),
            _ => None,
        };
        Params {
            name: data.name,
            domain: data.domain,
            invoice_api: data.backend_data.unwrap(),
            pin: data.pin.unwrap(),
            min_sendable,
            ..Default::default()
        }
    }
}

/// Validates that the domain is within domain list defined
/// in the config.
fn validate_domain(domain: &str, config: &Config) -> Result<(), ValidationError> {
    if !config.domains.contains(&domain.to_owned()) {
        return Err(ValidationError::new("domain not supported"));
    }
    Ok(())
}

/// Validates that the passed backend data is within the
/// supported list of backends (which InvoiceAPI enum represents)
fn validate_backend(backend: &str, _config: &Config) -> Result<(), ValidationError> {
    if !InvoiceAPI::iter()
        .map(|i| i.to_string())
        .any(|x| x == *backend)
    {
        return Err(ValidationError::new("backend not supported"));
    }
    Ok(())
}

#[derive(Error, Debug)]
enum Error {
    #[error("JSON path error: {0}")]
    JSONPath(String),
    #[error("validation error: {0}")]
    Validation(ValidationErrors),
    #[error("value error: {0}")]
    Val(String),
}

impl reject::Reject for Error {}

/// Main handler for requests from the web app.
pub async fn grab(db: Db, config: Config, buf: impl Buf) -> Result<impl Reply, Rejection> {
    let des = &mut serde_json::Deserializer::from_reader(buf.reader());
    let mut body: AliasPostData = serde_path_to_error::deserialize(des)
        .map_err(|e| reject::custom(Error::JSONPath(e.to_string())))?;

    debug!("processing the following body {:?}", body);

    // perform basic validation
    body.validate_args((&config, &config))
        .map_err(|e| reject::custom(Error::Validation(e)))?;

    // check for reserved username
    if config.reserved_names.contains(&body.name) {
        return Err(reject::custom(Error::Val(
            "trying to use a reserved username".to_string(),
        )));
    }

    // check if backend-specific data is correct
    match body.backend.as_str() {
        "Lnd" => {
            if let Some(InvoiceAPI::Lnd(ref params)) = body.backend_data {
                params
                    .validate()
                    .map_err(|e| reject::custom(Error::Validation(e)))?;
            } else {
                return Err(reject::custom(Error::Val(
                    "backend data not matching selection".to_string(),
                )));
            }
        }
        "LNBits" => {
            if let Some(InvoiceAPI::LNBits(ref params)) = body.backend_data {
                params
                    .validate()
                    .map_err(|e| reject::custom(Error::Validation(e)))?;
            } else {
                return Err(reject::custom(Error::Val(
                    "backend data not matching selection".to_string(),
                )));
            }
        }
        "Keysend" => {
            if let Some(InvoiceAPI::Keysend(ref params)) = body.backend_data {
                params
                    .validate()
                    .map_err(|e| reject::custom(Error::Validation(e)))?;
            } else {
                return Err(reject::custom(Error::Val(
                    "backend data not matching selection".to_string(),
                )));
            }
        }
        _ => {
            return Err(reject::custom(Error::Val(
                "wrong node backend data".to_string(),
            )))
        }
    }

    // get data out of db
    let entry = db
        .get(&body.name, &body.domain)
        .map_err(|e| reject::custom(Error::Val(e.to_string())))?;

    // check pin match if object exists
    let pin = compute_pin(&body.name, &body.domain, &config.pin_secret);
    if entry.is_some() {
        debug!("Generated pin to modify record = {:?}", pin);
        match body.pin {
            None => {
                return Err(reject::custom(Error::Val(
                    "PIN required to modify record (entry already exists)".to_string(),
                )))
            }
            Some(in_pin) if in_pin != pin => {
                return Err(reject::custom(Error::Val(
                    "provided PIN incorrect".to_string(),
                )))
            }
            Some(_) => (),
        }
    }

    // set the pin entry
    body.pin = Some(pin.clone());
    let mut params: Params = body.into();

    // we need to do some legwork when handling keysend
    if let InvoiceAPI::Keysend(ref mut k_params) = params.invoice_api {
        if let Some(Params {
            invoice_api: InvoiceAPI::Keysend(params),
            ..
        }) = entry
        {
            // update keysend pubkey if we only modify the entry
            let api_key = params.admin_key.unwrap();
            keysend::update_entry(&config.lnbits, &api_key, Some(&k_params.pub_key), None)
                .await
                .map_err(|e| Error::Val(format!("Problem updating pubkey: {}", e)))?;

            k_params.admin_key = Some(api_key);
        } else {
            // one-time: fully provision lnbits backend elements
            let (user_id, admin_key, wallet_id) = keysend::provision_backend(
                &config.lnbits,
                &params.name,
                &params.domain,
                &k_params.pub_key,
            )
            .await
            .map_err(|e| {
                reject::custom(Error::Val(format!(
                    "Problems with provision backend: {}",
                    e
                )))
            })?;
            k_params.user_id = Some(user_id);
            k_params.admin_key = Some(admin_key);
            k_params.wallet_id = Some(wallet_id);
        }
    }

    // try to generate the invoice
    let memo = format!("{}@{} PIN: {}", params.name, params.domain, pin);
    if let Err(e) = make_invoice(
        &params,
        &config.lnbits.url,
        42000,
        config.tor_proxy_url,
        Some(memo),
    )
    .await
    {
        error!("Problem with invoice generation {:?}", e);
        return Err(reject::custom(Error::Val(e.to_string())));
    }

    // update entry in the database
    db.insert(&params.name, &params.domain, &params)
        .map_err(|e| reject::custom(Error::Val(e.to_string())))?;

    let json = warp::reply::json(&json!({
        "message": "success",
        "pin": params.pin,
        "errors": [],
    }));
    Ok(warp::reply::with_status(json, StatusCode::CREATED))
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
    errors: Option<Vec<FieldError>>,
}

#[derive(Serialize)]
struct FieldError {
    field: String,
    field_errors: Vec<String>,
}

/// This function receives a `Rejection` and tries to return a custom
/// value, otherwise simply passes the rejection along.
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message, errors) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string(), None)
    } else if let Some(e) = err.find::<Error>() {
        match e {
            Error::Val(_) => (StatusCode::BAD_REQUEST, e.to_string(), None),
            Error::JSONPath(_) => (StatusCode::BAD_REQUEST, e.to_string(), None),
            Error::Validation(val_errs) => {
                let errors: Vec<FieldError> = val_errs
                    .errors()
                    .iter()
                    .map(|error_kind| FieldError {
                        field: error_kind.0.to_string(),
                        field_errors: match error_kind.1 {
                            ValidationErrorsKind::Struct(struct_err) => {
                                validation_errs_to_str_vec(struct_err)
                            }
                            ValidationErrorsKind::Field(field_errs) => field_errs
                                .iter()
                                .map(|fe| format!("{}: {:?}", fe.code, fe.params))
                                .collect(),
                            ValidationErrorsKind::List(vec_errs) => vec_errs
                                .iter()
                                .map(|ve| {
                                    format!(
                                        "{}: {:?}",
                                        ve.0,
                                        validation_errs_to_str_vec(ve.1).join(" | "),
                                    )
                                })
                                .collect(),
                        },
                    })
                    .collect();

                (
                    StatusCode::BAD_REQUEST,
                    "field errors".to_string(),
                    Some(errors),
                )
            }
        }
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        (
            StatusCode::BAD_REQUEST,
            e.source()
                .map(|cause| cause.to_string())
                .unwrap_or_else(|| "BAD_REQUEST".to_string()),
            None,
        )
    } else if let Some(e) = err.find::<LnUrlError>() {
        // generate response with error
        // so that we display it in the LNUrl client
        let success_action = SuccessAction {
            tag: "message".to_string(),
            ..Default::default()
        };

        let resp = LNURLPayValues {
            lnurl_response: LNURLResponse {
                status: Some("ERROR".to_string()),
                reason: Some(e.0.to_string()),
            },
            pr: json!({}),
            disposable: Some(false),
            success_action,
        };
        let json = warp::reply::json(&resp);
        return Ok(warp::reply::with_status(json, StatusCode::OK));
    } else {
        eprintln!("unhandled error: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
            None,
        )
    };

    let json = warp::reply::json(&ErrorResponse { message, errors });

    Ok(warp::reply::with_status(json, code))
}

fn validation_errs_to_str_vec(ve: &ValidationErrors) -> Vec<String> {
    ve.field_errors()
        .iter()
        .map(|fe| {
            format!(
                "{}: errors: {}",
                fe.0,
                fe.1.iter()
                    .map(|ve| format!("{}: {:?}", ve.code, ve.params))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        })
        .collect()
}

/// Computes the pin that is required in order to modify entries
/// via the API calls.
fn compute_pin(usnername: &str, domain: &str, secret: &str) -> String {
    let sha = Sha256::new()
        .chain_update(secret)
        .chain_update(usnername)
        .chain_update(domain)
        .finalize();

    hex::encode(sha)
}

#[cfg(test)]
mod tests {
    use super::compute_pin;

    #[test]
    fn computes_pin_for_user() {
        assert_eq!(
            compute_pin("user", "domain", "secret1"),
            "a8fe9f81a343e918a2aa9a6ee251b2e672c90b8f9b98d253db202ab910dc3668"
        );
    }
}
