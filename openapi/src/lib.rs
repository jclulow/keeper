use anyhow::Result;
mod progenitor_support {
    use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub(crate) fn encode_path(pc: &str) -> String {
        utf8_percent_encode(pc, PATH_SET).to_string()
    }
}

pub mod types {
    use serde::{Deserialize, Serialize};
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
        pub time: chrono::DateTime<chrono::offset::Utc>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct PingResult {
        pub host: String,
        pub ok: bool,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportFinishBody {
        pub duration_millis: u64,
        pub end_time: chrono::DateTime<chrono::offset::Utc>,
        pub exit_status: i32,
        pub id: ReportId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportId {
        pub host: String,
        pub job: String,
        pub pid: u32,
        pub time: chrono::DateTime<chrono::offset::Utc>,
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
        pub start_time: chrono::DateTime<chrono::offset::Utc>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReportSummary {
        pub age_seconds: i32,
        pub duration_seconds: i32,
        pub host: String,
        pub job: String,
        pub status: i32,
        pub when: chrono::DateTime<chrono::offset::Utc>,
    }
}

#[derive(Clone)]
pub struct Client {
    baseurl: String,
    client: reqwest::Client,
}

impl Client {
    pub fn new(baseurl: &str) -> Self {
        let dur = std::time::Duration::from_secs(15);
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(dur)
            .timeout(dur)
            .build()
            .unwrap();
        Self::new_with_client(baseurl, client)
    }

    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }

    pub fn baseurl(&self) -> &String {
        &self.baseurl
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    #[doc = "enrol: POST /enrol"]
    pub async fn enrol<'a>(
        &'a self,
        body: &'a types::EnrolBody,
    ) -> Result<reqwest::Response> {
        let url = format!("{}/enrol", self.baseurl,);
        let request = self.client.post(url).json(body).build()?;
        let result = self.client.execute(request).await;
        let res = result?.error_for_status()?;
        Ok(res)
    }

    #[doc = "global_jobs: GET /global/jobs"]
    pub async fn global_jobs<'a>(&'a self) -> Result<types::GlobalJobsResult> {
        let url = format!("{}/global/jobs", self.baseurl,);
        let request = self.client.get(url).build()?;
        let result = self.client.execute(request).await;
        let res = result?.error_for_status()?;
        Ok(res.json().await?)
    }

    #[doc = "ping: GET /ping"]
    pub async fn ping<'a>(&'a self) -> Result<types::PingResult> {
        let url = format!("{}/ping", self.baseurl,);
        let request = self.client.get(url).build()?;
        let result = self.client.execute(request).await;
        let res = result?.error_for_status()?;
        Ok(res.json().await?)
    }

    #[doc = "report_finish: POST /report/finish"]
    pub async fn report_finish<'a>(
        &'a self,
        body: &'a types::ReportFinishBody,
    ) -> Result<types::ReportResult> {
        let url = format!("{}/report/finish", self.baseurl,);
        let request = self.client.post(url).json(body).build()?;
        let result = self.client.execute(request).await;
        let res = result?.error_for_status()?;
        Ok(res.json().await?)
    }

    #[doc = "report_output: POST /report/output"]
    pub async fn report_output<'a>(
        &'a self,
        body: &'a types::ReportOutputBody,
    ) -> Result<types::ReportResult> {
        let url = format!("{}/report/output", self.baseurl,);
        let request = self.client.post(url).json(body).build()?;
        let result = self.client.execute(request).await;
        let res = result?.error_for_status()?;
        Ok(res.json().await?)
    }

    #[doc = "report_start: POST /report/start"]
    pub async fn report_start<'a>(
        &'a self,
        body: &'a types::ReportStartBody,
    ) -> Result<types::ReportResult> {
        let url = format!("{}/report/start", self.baseurl,);
        let request = self.client.post(url).json(body).build()?;
        let result = self.client.execute(request).await;
        let res = result?.error_for_status()?;
        Ok(res.json().await?)
    }
}
