use std::net::IpAddr;

use envconfig::Envconfig;
use warp::{hyper::Uri, Filter};

pub mod api;
pub mod db;
pub mod handlers;
pub mod ln;

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
    // pub inv_banner: Option<String>,
}

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

pub fn with_clone<C: Clone + Send>(
    c: C,
) -> impl Filter<Extract = (C,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || c.clone())
}
