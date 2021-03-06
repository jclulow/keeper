/*
 * Keeper API
 *
 * report execution of cron jobs through a mechanism other than mail
 *
 * The version of the OpenAPI document: 1.0
 * 
 * Generated by: https://openapi-generator.tech
 */




#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnrolBody {
    #[serde(rename = "host")]
    pub host: String,
    #[serde(rename = "key")]
    pub key: String,
}

impl EnrolBody {
    pub fn new(host: String, key: String) -> EnrolBody {
        EnrolBody {
            host,
            key,
        }
    }
}


