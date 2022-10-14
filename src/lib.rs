//! Federated Lightning Address Server
//!
//! The federated server allows you to easily handle LN Address
//! requests and add those capabilties to the domains you own.

use std::net::IpAddr;

use envconfig::Envconfig;
use warp::{hyper::Uri, Filter};

/// REST API responsible for admin tasks
pub mod api;
/// Abstraction over an embedded database
pub mod db;
/// Main web and api application handlers
pub mod handlers;
/// Lightning network helpers and structures
pub mod ln;

/// Structure definining possible params and their structure
/// used in order to configure the server
#[derive(Envconfig, Debug, Clone)]
pub struct Config {
    pub domains: CsvVec,
    #[envconfig(default = "127.0.0.1")]
    pub host: IpAddr,
    #[envconfig(default = "3030")]
    pub port: u16,

    #[envconfig(default = "admin,root,berni")]
    pub reserved_names: CsvVec,
    pub pin_secret: String,

    pub site_name: String,
    pub site_sub_name: String,
    #[envconfig(default = "socks5://127.0.0.1:9050")]
    pub tor_proxy_url: Uri,
}

/// Represents a comma delimited input for the CLI
/// and converts it into a vector of strings.
#[derive(Debug, Clone)]
pub struct CsvVec(Vec<String>);

impl CsvVec {
    pub fn contains(&self, s: &String) -> bool {
        self.0.contains(s)
    }
}

impl From<CsvVec> for Vec<String> {
    fn from(l: CsvVec) -> Self {
        l.0
    }
}

impl std::str::FromStr for CsvVec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s
            .trim()
            .split(',')
            .map(|s| s.trim())
            .map(String::from)
            .collect();

        Ok(CsvVec(v))
    }
}

/// Warp helper for cloning configration and db references
/// so they can be passed into request handlers.
pub fn with_clone<C: Clone + Send>(
    c: C,
) -> impl Filter<Extract = (C,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || c.clone())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{with_clone, CsvVec};

    #[tokio::test]
    async fn with_clone_returns_wrapped_clone() {
        let hello = "hello";
        let f = with_clone(hello);
        let value = warp::test::request().filter(&f).await.unwrap();
        assert_eq!(value, hello);
    }

    #[test]
    fn csv_vec_converts_str_to_vec() {
        let s = "elem1,elem2, elem3";
        let cv = CsvVec::from_str(s).unwrap();
        assert_eq!(cv.0, vec!["elem1", "elem2", "elem3"])
    }

    #[test]
    fn csv_vec_converts_to_vec() {
        let cv = CsvVec(vec!["elem".to_owned()]);
        let v: Vec<String> = cv.into();
        assert_eq!(v, vec!["elem"]);
    }
}
