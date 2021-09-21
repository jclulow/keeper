
use anyhow::Result;

mod progenitor_support {
    use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

    const PATH_SET: &AsciiSet = &CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'#')
        .add(b'<')
        .add(b'>')
        .add(b'?')
        .add(b'`')
        .add(b'{')
        .add(b'}');

    pub(crate) fn encode_path(pc: &str) -> String {
        utf8_percent_encode(pc, PATH_SET).to_string()
    }
}

pub mod types {
    use chrono::prelude::*;
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct EnrolBody {
        pub host: String,
        pub key: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct GlobalJobsResult {
        pub summary: Vec<ReportSummary>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct OutputRecord {
        pub msg: String,
        pub stream: String,
        pub time: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct PingResult {
        pub host: String,
        pub ok: bool,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportFinishBody {
        pub duration_millis: i64,
        pub end_time: DateTime<Utc>,
        pub exit_status: i64,
        pub id: ReportId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportId {
        pub host: String,
        pub job: String,
        pub pid: i64,
        pub time: DateTime<Utc>,
        pub uuid: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportOutputBody {
        pub id: ReportId,
        pub record: OutputRecord,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportResult {
        pub existed_already: bool,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportStartBody {
        pub id: ReportId,
        pub script: String,
        pub start_time: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportSummary {
        pub age_seconds: i64,
        pub duration_seconds: i64,
        pub host: String,
        pub job: String,
        pub status: i64,
        pub when: DateTime<Utc>,
    }

}

pub struct Client {
    baseurl: String,
    client: reqwest::Client,
}

impl Client {
    pub fn new(baseurl: &str) -> Client {
        let dur = std::time::Duration::from_secs(15);
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(dur)
            .timeout(dur)
            .build()
            .unwrap();

        Client::new_with_client(baseurl, client)
    }

    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Client {
        Client {
            baseurl: baseurl.to_string(),
            client,
        }
    }

    /**
     * enrol: POST /enrol
     */
    pub async fn enrol(
        &self,
        body: &types::EnrolBody,
    ) -> Result<()> {
        let url = format!("{}/enrol",
            self.baseurl,
        );

        let res = self.client.post(url)
            .json(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    /**
     * global_jobs: GET /global/jobs
     */
    pub async fn global_jobs(
        &self,
    ) -> Result<types::GlobalJobsResult> {
        let url = format!("{}/global/jobs",
            self.baseurl,
        );

        let res = self.client.get(url)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    /**
     * ping: GET /ping
     */
    pub async fn ping(
        &self,
    ) -> Result<types::PingResult> {
        let url = format!("{}/ping",
            self.baseurl,
        );

        let res = self.client.get(url)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    /**
     * report_finish: POST /report/finish
     */
    pub async fn report_finish(
        &self,
        body: &types::ReportFinishBody,
    ) -> Result<types::ReportResult> {
        let url = format!("{}/report/finish",
            self.baseurl,
        );

        let res = self.client.post(url)
            .json(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    /**
     * report_output: POST /report/output
     */
    pub async fn report_output(
        &self,
        body: &types::ReportOutputBody,
    ) -> Result<types::ReportResult> {
        let url = format!("{}/report/output",
            self.baseurl,
        );

        let res = self.client.post(url)
            .json(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    /**
     * report_start: POST /report/start
     */
    pub async fn report_start(
        &self,
        body: &types::ReportStartBody,
    ) -> Result<types::ReportResult> {
        let url = format!("{}/report/start",
            self.baseurl,
        );

        let res = self.client.post(url)
            .json(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

}
