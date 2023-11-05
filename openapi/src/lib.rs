use std::time::Duration;

use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

pub mod gen {
    progenitor::generate_api!(spec = "openapi.json", interface = Builder);
}

pub mod prelude {
    pub use super::gen::prelude::*;
}
pub use gen::{types, Client, Error};

pub struct ClientBuilder {
    url: String,
    token: Option<String>,
}

impl ClientBuilder {
    pub fn new(url: &str) -> ClientBuilder {
        ClientBuilder {
            url: url.to_string(),
            token: None,
        }
    }

    pub fn bearer_token<S: AsRef<str>>(&mut self, token: S) -> &mut Self {
        self.token = Some(token.as_ref().to_string());
        self
    }

    pub fn build(&mut self) -> Result<Client> {
        let mut dh = HeaderMap::new();

        if let Some(token) = self.token.as_deref() {
            dh.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            );
        }

        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(15))
            .tcp_keepalive(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .default_headers(dh)
            .build()?;

        Ok(Client::new_with_client(&self.url, client))
    }
}
