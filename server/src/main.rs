use std::path::PathBuf;
use anyhow::{Result, bail, anyhow, Context};
use getopts::Options;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::result::Result as SResult;
use std::sync::Arc;
use std::io::{Write, BufWriter};
use chrono::prelude::*;
#[allow(unused_imports)]
use slog::{debug, info, warn, error, Logger};
use std::any::Any;
use keeper_common::*;

use dropshot::{
    ConfigLogging,
    ConfigLoggingLevel,
    ConfigDropshot,
    RequestContext,
    ApiDescription,
    HttpServer,
    HttpError,
    HttpResponseCreated,
    endpoint,
    TypedBody,
};
use hyper::{StatusCode, header::AUTHORIZATION};

trait MakeInternalError<T> {
    fn or_500(self) -> SResult<T, HttpError>;
}

impl<T> MakeInternalError<T> for std::io::Result<T> {
    fn or_500(self) -> SResult<T, HttpError> {
        self.map_err(|e| {
            let msg = format!("internal error: {:?}", e);
            HttpError::for_internal_error(msg)
        })
    }
}

impl<T> MakeInternalError<T> for std::result::Result<T, anyhow::Error> {
    fn or_500(self) -> SResult<T, HttpError> {
        self.map_err(|e| {
            let msg = format!("internal error: {:?}", e);
            HttpError::for_internal_error(msg)
        })
    }
}

struct App {
    log: Logger,
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct KeyFile {
    host: String,
    key: String,
    time_create: DateTime<Utc>,
    #[serde(default)]
    global_view: bool,
}

/**
 * Host and job names must be safe as a filename, as we will store the
 * associated user account in "keys/<hostname>.json" and jobs are stored as
 * "reports/<hostname>/<jobname>/...".
 */
fn name_ok(n: &str) -> bool {
    n.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
    }) && n.len() >= 2
}

/**
 * Clients submit their own key during enrolment, and it will be included in
 * "Authorization" headers later.  Make sure the format is at least plausible.
 */
fn key_ok(k: &str) -> bool {
    k.chars().all(|c| { c.is_ascii_alphanumeric() }) && k.len() == 64
}

fn i64ton(v: i64) -> i32 {
    if v < 0 {
        0
    } else if v > std::i32::MAX as i64 {
        std::i32::MAX
    } else {
        v as i32
    }
}

fn u64ton(v: u64) -> i32 {
    if v > std::i32::MAX as u64 {
        std::i32::MAX
    } else {
        v as i32
    }
}

fn age_seconds(dt: &DateTime<Utc>) -> i32 {
    let dur = Utc::now().signed_duration_since(*dt);
    i64ton(dur.num_seconds())
}

#[derive(Serialize, JsonSchema)]
struct ReportSummary {
    host: String,
    job: String,
    when: DateTime<Utc>,
    status: i32,
    duration_seconds: i32,
    age_seconds: i32,
}

impl App {
    fn from_private(ctx: Arc<dyn Any + Send + Sync + 'static>) -> Arc<App> {
        ctx.downcast::<App>().expect("app downcast")
    }

    fn from_request(rqctx: &Arc<RequestContext>) -> Arc<App> {
        Self::from_private(Arc::clone(&rqctx.server.private))
    }

    fn summary(&self, perjob: usize) -> Result<Vec<ReportSummary>> {
        let mut out = Vec::new();

        for host in self.report_hosts()?.iter() {
            'job: for job in self.report_host_jobs(host)?.iter() {
                let mut c = 0usize;
                for y in self.report_hj_years(host, job)?.iter() {
                    if c >= perjob {
                        continue 'job;
                    }

                    for m in self.list_months(host, job, *y)?.iter() {
                        if c >= perjob {
                            continue 'job;
                        }

                        for d in self.list_days(host, job, *y, *m)?.iter() {
                            if c >= perjob {
                                continue 'job;
                            }

                            for r in
                                self.list_reports(host, job, *y, *m, *d)?.iter()
                            {
                                if c >= perjob {
                                    continue 'job;
                                }

                                let dt = Utc.timestamp_millis(*r);
                                let t = self.reportpath(host, job, &dt)?;

                                if let Ok(Some(p)) = load_file::<PostFile>(&t) {
                                    if p.sealed {
                                        let dur = p.duration_seconds();

                                        out.push(ReportSummary {
                                            host: host.to_string(),
                                            job: job.to_string(),
                                            age_seconds: age_seconds(&dt),
                                            duration_seconds: dur,
                                            when: dt,
                                            status: p.status.unwrap(),
                                        });
                                        c += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(out)
    }

    fn list_reports(&self, host: &str, job: &str, year: u32, month: u32,
        day: u32)
        -> Result<Vec<i64>>
    {
        let mut targ = self.dir.clone();
        targ.push("reports");
        targ.push(host);
        targ.push(job);
        targ.push(&format!("{:04}", year));
        targ.push(&format!("{:02}", month));
        targ.push(&format!("{:02}", day));

        let mut out = Vec::new();

        let mut dir = std::fs::read_dir(&targ).context("report reports")?;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_file() {
                continue;
            }

            if let Some(n) = ent.file_name().to_str() {
                if !n.ends_with(".json") {
                    continue;
                }

                if let Ok(num) = n.trim_end_matches(".json").parse::<i64>() {
                    if !out.contains(&num) {
                        out.push(num);
                    }
                }
            }
        }

        out.sort();
        out.reverse();
        Ok(out)
    }

    fn list_days(&self, host: &str, job: &str, year: u32, month: u32)
        -> Result<Vec<u32>>
    {
        let mut targ = self.dir.clone();
        targ.push("reports");
        targ.push(host);
        targ.push(job);
        targ.push(&format!("{:04}", year));
        targ.push(&format!("{:02}", month));

        let mut out = Vec::new();

        let mut dir = std::fs::read_dir(&targ).context("report days")?;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_dir() {
                continue;
            }

            if let Some(n) = ent.file_name().to_str() {
                if let Ok(num) = n.parse::<u32>() {
                    if !out.contains(&num) {
                        out.push(num);
                    }
                }
            }
        }

        out.sort();
        out.reverse();
        Ok(out)
    }

    fn list_months(&self, host: &str, job: &str, year: u32)
        -> Result<Vec<u32>>
    {
        let mut targ = self.dir.clone();
        targ.push("reports");
        targ.push(host);
        targ.push(job);
        targ.push(&format!("{:04}", year));

        let mut out = Vec::new();

        let mut dir = std::fs::read_dir(&targ).context("report months")?;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_dir() {
                continue;
            }

            if let Some(n) = ent.file_name().to_str() {
                if let Ok(num) = n.parse::<u32>() {
                    if !out.contains(&num) {
                        out.push(num);
                    }
                }
            }
        }

        out.sort();
        out.reverse();
        Ok(out)
    }

    fn report_hj_years(&self, host: &str, job: &str) -> Result<Vec<u32>> {
        let mut targ = self.dir.clone();
        targ.push("reports");
        targ.push(host);
        targ.push(job);

        let mut out = Vec::new();

        let mut dir = std::fs::read_dir(&targ).context("report years")?;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_dir() {
                continue;
            }

            if let Some(n) = ent.file_name().to_str() {
                if let Ok(num) = n.parse::<u32>() {
                    if !out.contains(&num) {
                        out.push(num);
                    }
                }
            }
        }

        out.sort();
        out.reverse();
        Ok(out)
    }

    fn report_host_jobs(&self, host: &str) -> Result<Vec<String>> {
        let mut targ = self.dir.clone();
        targ.push("reports");
        targ.push(host);

        let mut out = Vec::new();

        let mut dir = std::fs::read_dir(&targ).context("report jobs")?;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_dir() {
                continue;
            }

            if let Some(n) = ent.file_name().to_str() {
                /*
                 * NB: directory entry names should be unique, so we don't check
                 * here.
                 */
                out.push(n.to_string());
            }
        }

        out.sort();
        Ok(out)
    }

    fn report_hosts(&self) -> Result<Vec<String>> {
        let mut targ = self.dir.clone();
        targ.push("reports");

        let mut out = Vec::new();

        let mut dir = std::fs::read_dir(&targ).context("report hosts")?;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_dir() {
                continue;
            }

            if let Some(n) = ent.file_name().to_str() {
                /*
                 * NB: directory entry names should be unique, so we don't check
                 * here.
                 */
                out.push(n.to_string());
            }
        }

        out.sort();
        Ok(out)
    }

    fn reportpath(&self, host: &str, job: &str, time: &DateTime<Utc>)
        -> Result<PathBuf>
    {
        if !name_ok(host) || !name_ok(job) {
            bail!("invalid host or job name");
        }

        let mut targ = self.dir.clone();
        targ.push("reports");
        targ.push(host);
        targ.push(job);
        targ.push(time.format("%Y").to_string());
        targ.push(time.format("%m").to_string());
        targ.push(time.format("%d").to_string());

        debug!(self.log, "creating report directory: {}", targ.display());
        std::fs::create_dir_all(&targ)?;

        targ.push(format!("{}.json", time.timestamp_millis()));

        Ok(targ)
    }

    fn keypath(&self, set: &str, host: &str) -> Result<PathBuf> {
        if !name_ok(host) {
            bail!("invalid hostname");
        }

        let mut kpath = self.dir.clone();
        kpath.push(set);
        std::fs::create_dir_all(&kpath)?;
        kpath.push(&format!("{}.json", host));

        Ok(kpath)
    }

    fn check_key(&self, host: &str, key: &str) -> Result<Option<Auth>> {
        if !name_ok(host) {
            return Ok(None);
        }
        let kpath = self.keypath("keys", host)?;

        let kf: KeyFile = if let Some(kf) = load_file(&kpath)? {
            kf
        } else {
            return Ok(None);
        };

        if host != &kf.host {
            bail!("key file {} has wrong host {}", kpath.display(), kf.host);
        }

        Ok(if key == &kf.key {
            Some(Auth {
                host: kf.host.to_string(),
                global_view: kf.global_view,
            })
        } else {
            None
        })
    }

    fn enrol_key(&self, host: &str, key: &str) -> Result<bool> {
        if !name_ok(host) || !key_ok(key) {
            return Ok(false);
        }

        /*
         * If we have already confirmed this host, let's log a warning but
         * pretend to the client that everything was OK.
         * XXX This is obviously a bit of a race.
         */
        match std::fs::metadata(&self.keypath("keys", host)?) {
            Ok(f) if f.is_file() => {
                warn!(self.log, "re-enrolment for already confirmed host {}",
                    host);
                return Ok(true);
            }
            _ => {}
        }

        let kpath = self.keypath("enrol", host)?;

        let mut buf = serde_json::to_vec_pretty(&KeyFile {
            host: host.to_string(),
            key: key.to_string(),
            time_create: Utc::now(),
            global_view: false,
        })?;
        buf.push(b'\n');

        let fres = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&kpath);
        let f = match fres {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                /*
                 * Assume the host was over-writing a previous registration
                 * attempt, without giving away the possibly sensitive fact that
                 * such a registration occurred to others.
                 */
                return Ok(true);
            }
            Err(e) => bail!("enrol key failure: {:?}", e),
        };
        let mut bw = BufWriter::new(f);
        bw.write_all(&mut buf)?;
        bw.flush()?;
        Ok(true)
    }
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq)]
struct OutputRecord {
    time: DateTime<Utc>,
    stream: String,
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct PostFile {
    report_pid: u64,
    report_uuid: String,
    report_time: DateTime<Utc>,
    time_start: DateTime<Utc>,
    time_end: Option<DateTime<Utc>>,
    script: String,
    duration: Option<u64>,
    status: Option<i32>,
    #[serde(default)]
    output: Vec<OutputRecord>,
    #[serde(default)]
    sealed: bool,
}

impl PostFile {
    fn duration_seconds(&self) -> i32 {
        u64ton(self.duration.unwrap() / 1000)
    }
}

struct Auth {
    host: String,
    global_view: bool,
}

trait RequestBodyExt {
    fn require_auth(&self, app: &App)
        -> SResult<Auth, HttpError>;
}

impl RequestBodyExt for hyper::Request<hyper::Body> {
    fn require_auth(&self, app: &App)
        -> SResult<Auth, HttpError>
    {
        let v = if let Some(h) = self.headers().get(AUTHORIZATION) {
            if let Ok(v) = h.to_str() {
                Some(v.to_string())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(v) = v {
            let t = v.split_whitespace().map(|s| s.trim()).collect::<Vec<_>>();

            if t.len() == 2 && t.iter().all(|s| !s.is_empty()) {
                match app.check_key(t[0], t[1]) {
                    Ok(Some(auth)) => return Ok(auth),
                    Ok(None) => (),
                    Err(e) => {
                        let msg = format!("internal error: {:?}", e);
                        return Err(HttpError::for_internal_error(msg));
                    }
                }
            }
        }

        Err(HttpError::for_client_error(None, StatusCode::UNAUTHORIZED,
            "invalid Authorization header".into()))
    }
}

#[derive(Deserialize, JsonSchema)]
struct EnrolBody {
    host: String,
    key: String,
}

#[endpoint {
    method = POST,
    path = "/enrol",
}]
async fn enrol(
    arc: Arc<RequestContext>,
    body: TypedBody<EnrolBody>)
    -> SResult<HttpResponseCreated<()>, HttpError>
{
    let body = body.into_inner();
    let app = App::from_request(&arc);

    if !key_ok(&body.key) {
        return Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "invalid key format".into()));
    }
    if !name_ok(&body.host) {
        return Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "invalid name format".into()));
    }

    if app.enrol_key(&body.host, &body.key).or_500()? {
        Ok(HttpResponseCreated(()))
    } else {
        Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "invalid enrolment request".into()))
    }
}

#[derive(Deserialize, JsonSchema)]
struct ReportId {
    host: String,
    job: String,
    pid: u64,
    time: DateTime<Utc>,
    uuid: String,
}

#[derive(Deserialize, JsonSchema)]
struct ReportStartBody {
    id: ReportId,
    start_time: DateTime<Utc>,
    script: String,
}

#[derive(Serialize, JsonSchema)]
struct ReportResult {
    existed_already: bool,
}

#[endpoint {
    method = POST,
    path = "/report/start",
}]
async fn report_start(
    arc: Arc<RequestContext>,
    body: TypedBody<ReportStartBody>)
    -> SResult<HttpResponseCreated<ReportResult>, HttpError>
{
    let body = body.into_inner();
    let app = App::from_request(&arc);

    let auth = arc.request.lock().await.require_auth(&app)?;
    if body.id.host != auth.host {
        return Err(HttpError::for_client_error(None, StatusCode::UNAUTHORIZED,
            "uh uh uh".into()));
    }

    if !name_ok(&body.id.job) {
        return Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "job name too short".into()));
    }
    /*
     * XXX check that job time is in the last fornight, or whatever
     */

    let targ = app.reportpath(&body.id.host, &body.id.job, &body.id.time)
        .or_500()?;

    match load_file::<PostFile>(&targ) {
        Ok(Some(f)) => {
            /*
             * A report for this time exists already.  Check to make sure that
             * the report UUID is the same as what the client sent; if it is, we
             * can return success, but if not we should return a conflict.
             */
            if body.id.uuid != f.report_uuid {
                Err(HttpError::for_client_error(None,
                    StatusCode::CONFLICT,
                    "this time already submitted, with different UUID".into()))
            } else if f.sealed {
                Err(HttpError::for_client_error(None,
                    StatusCode::CONFLICT,
                    "this job is already complete".into()))
            } else {
                Ok(HttpResponseCreated(ReportResult {
                    existed_already: true,
                }))
            }
        }
        Ok(None) => {
            /*
             * A report for this time does not exist, so we can accept what the
             * client has sent!
             */
            let pf = PostFile {
                sealed: false,
                report_uuid: body.id.uuid,
                report_time: Utc::now(),
                report_pid: body.id.pid,
                time_start: body.start_time,
                time_end: None,
                duration: None,
                status: None,
                output: Vec::new(),
                script: body.script,
            };
            if let Err(e) = store_file(&targ, &pf, false) {
                Err(HttpError::for_internal_error(
                    format!("store file? {:?}", e)))
            } else {
                Ok(HttpResponseCreated(ReportResult {
                    existed_already: false,
                }))
            }
        }
        Err(e) => {
            error!(arc.log, "load file error: {:?}", e);
            Err(HttpError::for_internal_error("data store error".into()))
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct ReportOutputBody {
    id: ReportId,
    record: OutputRecord,
}

#[endpoint {
    method = POST,
    path = "/report/output",
}]
async fn report_output(
    arc: Arc<RequestContext>,
    body: TypedBody<ReportOutputBody>)
    -> SResult<HttpResponseCreated<ReportResult>, HttpError>
{
    let body = body.into_inner();
    let app = App::from_request(&arc);

    let auth = arc.request.lock().await.require_auth(&app)?;
    if body.id.host != auth.host {
        return Err(HttpError::for_client_error(None, StatusCode::UNAUTHORIZED,
            "uh uh uh".into()));
    }

    if !name_ok(&body.id.job) {
        return Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "job name too short".into()));
    }

    /*
     * XXX check that job time is in the last fornight, or whatever
     */
    let targ = app.reportpath(&body.id.host, &body.id.job, &body.id.time)
        .or_500()?;

    match load_file::<PostFile>(&targ) {
        Ok(Some(mut f)) => {
            /*
             * A report for this time exists already.  Check to make sure that
             * the report UUID is the same as what the client sent; if it is, we
             * can return success, but if not we should return a conflict.
             */
            if body.id.uuid != f.report_uuid {
                Err(HttpError::for_client_error(None,
                    StatusCode::CONFLICT,
                    "this time already submitted, with different UUID".into()))
            } else if f.sealed {
                Err(HttpError::for_client_error(None,
                    StatusCode::CONFLICT,
                    "this job is already complete".into()))
            } else {
                /*
                 * This job exists and the UUID matches the one recorded when
                 * the record was created.  Check to make sure the output
                 * record does not already appear in the file.
                 */
                if f.output.contains(&body.record) {
                    Ok(HttpResponseCreated(ReportResult {
                        existed_already: true,
                    }))
                } else {
                    f.output.push(body.record);

                    if let Err(e) = store_file(&targ, &f, false) {
                        Err(HttpError::for_internal_error(
                            format!("store file? {:?}", e)))
                    } else {
                        Ok(HttpResponseCreated(ReportResult {
                            existed_already: false,
                        }))
                    }
                }
            }
        }
        Ok(None) => {
            /*
             * If the job file does not exist already, we cannot append an
             * output record to it.
             */
            Err(HttpError::for_client_error(None,
                StatusCode::BAD_REQUEST,
                "this job does not exist".into()))
        }
        Err(e) => {
            error!(arc.log, "load file error: {:?}", e);
            Err(HttpError::for_internal_error("data store error".into()))
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct ReportFinishBody {
    id: ReportId,

    end_time: DateTime<Utc>,
    duration_millis: i32,
    exit_status: i32,
}

#[endpoint {
    method = POST,
    path = "/report/finish",
}]
async fn report_finish(
    arc: Arc<RequestContext>,
    body: TypedBody<ReportFinishBody>)
    -> SResult<HttpResponseCreated<ReportResult>, HttpError>
{
    let body = body.into_inner();
    let app = App::from_request(&arc);

    let auth = arc.request.lock().await.require_auth(&app)?;
    if body.id.host != auth.host {
        return Err(HttpError::for_client_error(None, StatusCode::UNAUTHORIZED,
            "uh uh uh".into()));
    }

    if !name_ok(&body.id.job) {
        return Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "job name too short".into()));
    }

    /*
     * XXX check that job time is in the last fornight, or whatever
     */
    let targ = app.reportpath(&body.id.host, &body.id.job, &body.id.time)
        .or_500()?;

    match load_file::<PostFile>(&targ) {
        Ok(Some(mut f)) => {
            /*
             * A report for this time exists already.  Check to make sure that
             * the report UUID is the same as what the client sent; if it is, we
             * can return success, but if not we should return a conflict.
             */
            if body.id.uuid != f.report_uuid {
                Err(HttpError::for_client_error(None,
                    StatusCode::CONFLICT,
                    "this time already submitted, with different UUID".into()))
            } else if f.sealed {
                Ok(HttpResponseCreated(ReportResult {
                    existed_already: true,
                }))
            } else {
                f.duration = Some(body.duration_millis as u64);
                f.time_end = Some(body.end_time);
                f.status = Some(body.exit_status);
                f.sealed = true;

                if let Err(e) = store_file(&targ, &f, false) {
                    Err(HttpError::for_internal_error(
                        format!("store file? {:?}", e)))
                } else {
                    Ok(HttpResponseCreated(ReportResult {
                        existed_already: false,
                    }))
                }
            }
        }
        Ok(None) => {
            /*
             * If the job file does not exist already, we cannot append an
             * output record to it.
             */
            Err(HttpError::for_client_error(None,
                StatusCode::BAD_REQUEST,
                "this job does not exist".into()))
        }
        Err(e) => {
            error!(arc.log, "load file error: {:?}", e);
            Err(HttpError::for_internal_error("data store error".into()))
        }
    }
}

#[derive(Serialize, JsonSchema)]
struct GlobalJobsResult {
    summary: Vec<ReportSummary>,
}

#[endpoint {
    method = GET,
    path = "/global/jobs",
}]
async fn global_jobs(
    arc: Arc<RequestContext>,
) -> SResult<HttpResponseCreated<GlobalJobsResult>, HttpError>
{
    let app = App::from_request(&arc);

    let auth = arc.request.lock().await.require_auth(&app)?;
    if !auth.global_view {
        return Err(HttpError::for_client_error(None, StatusCode::UNAUTHORIZED,
            "uh uh uh".into()));
    }

    let summary = app.summary(1).or_500()?;

    Ok(HttpResponseCreated(GlobalJobsResult {
        summary,
    }))
}


#[tokio::main]
async fn main() -> Result<()> {
    let mut opts = Options::new();

    opts.optopt("b", "", "bind address:port", "BIND_ADDRESS");
    opts.optopt("d", "", "data directory", "DIRECTORY");
    opts.optopt("S", "", "dump OpenAPI schema", "FILE");

    let p = match opts.parse(std::env::args().skip(1)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: usage: {}", e);
            eprintln!("       {}", opts.usage("usage"));
            std::process::exit(1);
        }
    };

    let mut api = ApiDescription::new();
    api.register(enrol).unwrap();
    api.register(report_start).unwrap();
    api.register(report_output).unwrap();
    api.register(report_finish).unwrap();
    api.register(global_jobs).unwrap();

    if let Some(s) = p.opt_str("S") {
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&s)?;
        api.openapi("Keeper API", "1.0")
            .description("report execution of cron jobs through a \
                mechanism other than mail")
            .contact_name("Joshua M. Clulow")
            .contact_url("https://github.com/jclulow/keeper")
            .write(&mut f)?;
        return Ok(());
    }

    let bind = p.opt_str("b").unwrap_or_else(|| String::from("0.0.0.0:9978"));
    let dir = if let Some(d) = p.opt_str("d") {
        PathBuf::from(d)
    } else {
        bail!("ERROR: must specify data directory (-d)");
    };
    if !dir.is_dir() {
        bail!("ERROR: {} should be a directory", dir.display());
    }

    let cfglog = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = cfglog.to_logger("keeper")?;

    let app = Arc::new(App {
        log: log.clone(),
        dir,
    });

    let cfgds = ConfigDropshot {
        bind_address: bind.parse()?,
        ..Default::default()
    };

    let mut server = HttpServer::new(&cfgds, api, app, &log)?;
    let task = server.run();
    server.wait_for_shutdown(task).await
        .map_err(|e| anyhow!("server task failure: {:?}", e))?;
    bail!("early exit is unexpected");
}
