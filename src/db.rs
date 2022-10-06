use log::debug;
use std::env;

use anyhow::{bail, Result};

use self::models::Params;

pub static DEFAULT_NAME: &str = "sataddress.db";
pub struct Db(sled::Db);

impl Clone for Db {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Db {
    pub fn from_path(path: &str) -> Result<Self> {
        Ok(Self(sled::open(path)?))
    }

    pub fn init() -> Result<Self> {
        let db = Db::from_path(DEFAULT_NAME)?;

        // print db data in case we're in the debug mode
        if env::var_os("RUST_LOG").unwrap_or_else(|| "".into()) == "debug" {
            for r in db.0.iter() {
                let ivec = r.unwrap();
                let p: Params = rmp_serde::from_slice(&ivec.1).unwrap();
                debug!("{}@{}: stats => {:?}", p.name, p.domain, p.stats);
            }
        }

        Ok(db)
    }

    pub fn clear(&self) -> Result<(), sled::Error> {
        self.0.clear()
    }

    pub fn iter(&self) -> sled::Iter {
        self.0.iter()
    }

    pub fn insert(&self, username: &str, domain: &str, params: &Params) -> Result<Option<()>> {
        let key = format!("{}@{}", username, domain);
        let value = rmp_serde::to_vec_named(params)?;
        match self.0.insert(key, value)? {
            Some(_) => Ok(Some(())),
            None => Ok(None),
        }
    }

    pub fn update(&self, params: &Params) -> Result<()> {
        let key = &format!("{}@{}", params.name, params.domain);
        if !self.0.contains_key(key)? {
            bail!("Key does not exist: {}", key);
        }

        let value = rmp_serde::to_vec_named(params)?;
        self.0.insert(key, value)?;
        Ok(())
    }

    pub fn get(&self, username: &str, domain: &str) -> Result<Option<Params>> {
        let key = format!("{}@{}", username, domain);
        let ivec = self.0.get(key)?;

        match ivec {
            Some(ivec) => Ok(rmp_serde::from_slice(&ivec)?),
            None => Ok(None),
        }
    }
}

pub mod defaults {
    pub static MIN_SENDABLE: u64 = 1_000;
    pub static MAX_SENDABLE: u64 = 1_000_000_000;
}

pub mod models {
    use std::{cmp::Ordering, time::SystemTime};

    use serde::{Deserialize, Serialize};
    use strum_macros::{self, Display, EnumIter};

    use validator::Validate;

    #[derive(Serialize, Deserialize, Debug, Clone, EnumIter, Display)]
    pub enum InvoiceAPI {
        Lnd(LNDParams),
        LNBits(LNBitsParams),
    }

    impl Default for InvoiceAPI {
        fn default() -> Self {
            Self::Lnd(LNDParams {
                host: "".to_string(),
                macaroon: "".to_string(),
            })
        }
    }

    impl InvoiceAPI {
        pub fn is_tor(&self) -> bool {
            match self {
                InvoiceAPI::Lnd(p) => p.host.contains(".onion"),
                InvoiceAPI::LNBits(p) => p.host.contains(".onion"),
            }
        }
        pub fn get_comment_len(&self) -> u8 {
            // lnbits invoice api implementation does not allow having
            // both memo and unhashed_description, it prefers unhashed_desc if both are provided
            // https://github.com/lnbits/lnbits/blob/1660b9dcf1f3c17af1b7d7a894f6ce06359ca578/lnbits/core/views/api.py#L153
            // which leads to problems:
            // - if we use memo client wallet complains that there's no desc_hash
            // - if we use unhashed_desc, there's no memo visible on lnbits when recieving
            // sending from LND works though as it does not expect unhashed_desc, ugh
            match self {
                InvoiceAPI::Lnd(_) => 128,
                InvoiceAPI::LNBits(_) => 0,
            }
        }
    }

    #[derive(Serialize, Deserialize, Validate, Debug, Default, Clone)]
    pub struct LNDParams {
        #[validate(url)]
        pub host: String,
        #[validate(length(min = 1))]
        pub macaroon: String,
    }

    #[derive(Serialize, Deserialize, Validate, Debug, Default, Clone)]
    pub struct LNBitsParams {
        #[validate(url)]
        pub host: String,
        #[validate(length(min = 1))]
        pub key: String,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct Counter {
        pub num: u16,
        pub last_update: SystemTime,
    }

    impl Counter {
        pub fn inc(&mut self) {
            self.num += 1;
            self.last_update = SystemTime::now();
        }
    }

    impl Default for Counter {
        fn default() -> Self {
            Self {
                num: Default::default(),
                last_update: SystemTime::now(),
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize, Default, Clone)]
    pub struct Stats {
        pub invoices: Counter,
        pub calls: Counter,
        pub edits: Counter,
    }

    impl Ord for Stats {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            let this = self.invoices.num + self.calls.num + self.edits.num;
            let other = other.invoices.num + other.calls.num + other.edits.num;
            this.cmp(&other)
        }
    }

    impl PartialOrd for Stats {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl PartialEq for Stats {
        fn eq(&self, other: &Self) -> bool {
            self.cmp(other) == Ordering::Equal
        }
    }

    impl Eq for Stats {}

    #[derive(Debug, Deserialize, Serialize, Default, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Params {
        pub name: String,
        pub domain: String,
        pub invoice_api: InvoiceAPI,
        pub min_sendable: Option<u64>,
        pub max_sendable: Option<u64>,

        pub pin: String,
        #[serde(default)]
        pub stats: Stats,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
