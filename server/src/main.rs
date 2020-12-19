use std::path::{Path, PathBuf};
use anyhow::{Result, bail, anyhow};
use getopts::Options;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::result::Result as SResult;
use std::sync::Arc;
use std::io::{Read, BufReader, Write, BufWriter};
use chrono::prelude::*;
use slog::{info, error};
use std::any::Any;

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
};
use hyper::StatusCode;

struct App {
    dir: PathBuf,
}

impl App {
    fn from_private(ctx: Arc<dyn Any + Send + Sync + 'static>) -> Arc<App> {
        ctx.downcast::<App>().expect("app downcast")
    }

    fn from_request(rqctx: &Arc<RequestContext>) -> Arc<App> {
        Self::from_private(Arc::clone(&rqctx.server.private))
    }
}

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

#[derive(Serialize, Deserialize)]
struct PostFile {
    report_uuid: String,
    report_time: i64,
    ok: bool,
    status: Option<u64>,
    stdout: Option<String>,
    stderr: Option<String>,
}

fn load_file<T>(p: &Path)
    -> Result<Option<T>>
    where for<'de> T: Deserialize<'de>,
{
    let f = match std::fs::File::open(p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => bail!("load file {}: {:?}", p.display(), e),
        Ok(f) => f,
    };
    let mut br = BufReader::new(f);
    let mut buf: Vec<u8> = Vec::new();
    br.read_to_end(&mut buf)?;
    Ok(serde_json::from_slice(buf.as_slice())?)
}

fn store_file<T>(p: &Path, data: &T)
    -> Result<()>
    where T: Serialize,
{
    let mut tmp = p.to_path_buf();
    let mut n = p.file_name().unwrap().to_os_string();
    n.push(".tmp");
    tmp.set_file_name(n);

    let f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)?;
    let mut bw = BufWriter::new(f);
    let mut buf = serde_json::to_vec(data)?;
    bw.write_all(&mut buf)?;
    bw.flush()?;

    std::fs::rename(&tmp, p)?;
    Ok(())
}
 
 
#[derive(Deserialize, JsonSchema)]
struct ReportPostArgs {
    host: String,
    job: String,
    time: u64,
}

#[derive(Deserialize, JsonSchema)]
struct ReportPostBody {
    uuid: String,
    ok: bool,
    status: Option<u64>,
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct ReportPostResult {
    ok: bool,
    existed_already: bool,
}

#[endpoint {
    method = PUT,
    path = "/report/{host}/{job}/{time}",
}]
async fn report_post(
    arc: Arc<RequestContext>,
    path: dropshot::Path<ReportPostArgs>,
    body: dropshot::TypedBody<ReportPostBody>)
    -> SResult<HttpResponseCreated<ReportPostResult>, HttpError>
{
    let path = path.into_inner();
    let body = body.into_inner();
    let app = App::from_request(&arc);

    /*
     * XXX check authentication for host
     */
    if &path.host != "sigma" {
        return Err(HttpError::for_client_error(None, StatusCode::UNAUTHORIZED,
            "uh uh uh".into()));
    }
    if path.job.len() < 5 {
        return Err(HttpError::for_client_error(None, StatusCode::BAD_REQUEST,
            "job name too short".into()));
    }
    /*
     * XXX check that job time is in the last fornight, or whatever
     */

    let dt = Utc.timestamp_millis(path.time as i64);

    let mut targ = app.dir.clone();
    targ.push("reports");
    targ.push(&path.host);
    targ.push(&path.job);
    targ.push(dt.format("%Y").to_string());
    targ.push(dt.format("%m").to_string());
    targ.push(dt.format("%d").to_string());

    info!(arc.log, "creating report directory: {}", targ.display());
    std::fs::create_dir_all(&targ).or_500()?;

    targ.push(format!("{}.json", path.time));

    match load_file::<PostFile>(&targ) {
        Ok(Some(f)) => {
            /*
             * A report for this time exists already.  Check to make sure that
             * the report UUID is the same as what the client sent; if it is, we
             * can return success, but if not we should return a conflict.
             */
            if body.uuid != f.report_uuid {
                Err(HttpError::for_client_error(None,
                    StatusCode::CONFLICT,
                    "this time already submitted, with different UUID".into()))
            } else {
                Ok(HttpResponseCreated(ReportPostResult {
                    ok: true,
                    existed_already: true,
                }))
            }
        }
        Ok(None) => {
            /*
             * A report for this time does not exist, so we can accept what the
             * client has sent!
             */
            let report_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            let pf = PostFile {
                report_time,
                report_uuid: body.uuid,
                ok: body.ok,
                status: body.status,
                stderr: body.stderr,
                stdout: body.stdout,
            };
            if let Err(e) = store_file(&targ, &pf) {
                Err(HttpError::for_internal_error(
                    format!("store file? {:?}", e)))
            } else {
                Ok(HttpResponseCreated(ReportPostResult {
                    ok: true,
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

#[tokio::main]
async fn main() -> Result<()> {
    let mut opts = Options::new();

    opts.optopt("b", "", "bind address:port", "BIND_ADDRESS");
    opts.reqopt("d", "", "data directory", "DIRECTORY");

    let p = match opts.parse(std::env::args().skip(1)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: usage: {}", e);
            eprintln!("       {}", opts.usage("usage"));
            std::process::exit(1);
        }
    };

    let bind = p.opt_str("b").unwrap_or(String::from("0.0.0.0:9978"));
    let dir = PathBuf::from(p.opt_str("d").unwrap());
    if !dir.is_dir() {
        bail!("ERROR: {} should be a directory", dir.display());
    }

    let cfglog = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = cfglog.to_logger("keeper")?;

    let app = Arc::new(App {
        dir,
    });

    let mut api = ApiDescription::new();
    api.register(report_post).unwrap();

    let cfgds = ConfigDropshot {
        bind_address: bind.parse()?,
    };

    let mut server = HttpServer::new(&cfgds, api, app, &log)?;
    let task = server.run();
    server.wait_for_shutdown(task).await
        .map_err(|e| anyhow!("server task failure: {:?}", e))?;
    bail!("early exit is unexpected");
}
