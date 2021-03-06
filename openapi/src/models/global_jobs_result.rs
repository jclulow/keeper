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
pub struct GlobalJobsResult {
    #[serde(rename = "summary")]
    pub summary: Vec<crate::models::ReportSummary>,
}

impl GlobalJobsResult {
    pub fn new(summary: Vec<crate::models::ReportSummary>) -> GlobalJobsResult {
        GlobalJobsResult {
            summary,
        }
    }
}


