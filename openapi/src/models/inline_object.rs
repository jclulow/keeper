/*
 * Keeper API
 *
 * No description provided (generated by Openapi Generator https://github.com/openapitools/openapi-generator)
 *
 * The version of the OpenAPI document: 1.0
 * 
 * Generated by: https://openapi-generator.tech
 */




#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InlineObject {
    #[serde(rename = "host")]
    pub host: String,
    #[serde(rename = "key")]
    pub key: String,
}

impl InlineObject {
    pub fn new(host: String, key: String) -> InlineObject {
        InlineObject {
            host,
            key,
        }
    }
}


