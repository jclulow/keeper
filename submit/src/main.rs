use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use chrono::prelude::*;
use hiercmd::prelude::*;
use keeper_common::*;
use keeper_openapi::{types::*, Client};
use serde::{Deserialize, Serialize};

mod exec;
use exec::Activity;

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    baseurl: String,
    host: String,
    key: String,
}

fn make_client(cf: &ConfigFile) -> Result<Client> {
    keeper_openapi::ClientBuilder::new(&cf.baseurl)
        .bearer_token(&cf.key)
        .build()
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut l = Level::new("keeper-submit", ());

    l.cmd(
        "enrol",
        "enrol the client with a keeper server",
        cmd!(cmd_enrol),
    )?;
    l.cmd("ping", "ping the keeper server", cmd!(cmd_ping))?;
    l.cmd("exec", "execute a job under keeper control", cmd!(cmd_exec))?;
    l.cmd(
        "cron",
        "like exec, but for cron; no stdio output is generated",
        cmd!(cmd_cron),
    )?;

    sel!(l).run().await
}

struct LoadedConfig {
    path: PathBuf,
    config: Option<ConfigFile>,
}

fn load_config() -> Result<LoadedConfig> {
    let path = if let Some(mut home) = dirs::home_dir() {
        home.push(".keeper.json");
        home
    } else {
        bail!("could not find home directory");
    };

    Ok(LoadedConfig {
        config: load_file(&path)?,
        path,
    })
}

async fn cmd_enrol(mut l: Level<()>) -> Result<()> {
    l.usage_args(Some("NODENAME URL"));

    let a = args!(l);

    let lc = load_config()?;

    if a.args().len() != 2 {
        bad_args!(l, "specify a host name, and a keeper URL for enrolment");
    }
    let host = a.args()[0].to_string();
    let baseurl = a.args()[1].to_string();

    let cf = if let Some(cf) = lc.config {
        if host != cf.host || baseurl != cf.baseurl {
            bail!("conflicting enrolment already exists");
        }
        cf
    } else {
        let cf = ConfigFile {
            baseurl,
            host,
            key: genkey(64),
        };
        store_file(&lc.path, &cf, true)?;
        cf
    };

    let c = make_client(&cf)?;

    let body = EnrolBody {
        host: cf.host.to_string(),
        key: cf.key.to_string(),
    };

    loop {
        match c.enrol().body(&body).send().await {
            Ok(_) => {
                println!("ok");
                return Ok(());
            }
            /*
             * XXX progenitor needs a real error type
             *
            Err(Error::ResponseError(e)) => {
                let status = e.status.as_u16();
                if status >= 400 && status <= 499 {
                    bail!("request error; giving up: {:?}", e);
                } else {
                    eprintln!("request error; retrying: {:?}", e);
                }
            }
             * XXX
             */
            Err(e) => {
                eprintln!("other error; retrying: {:?}", e);
            }
        }

        sleep_ms(1000);
    }
}

async fn cmd_ping(mut l: Level<()>) -> Result<()> {
    no_args!(l);

    let lc = load_config()?;
    let cf = lc
        .config
        .as_ref()
        .ok_or_else(|| anyhow!("no configuration file; enrol first"))?;
    let c = make_client(cf)?;

    loop {
        match c.ping().send().await {
            Ok(p) => {
                if p.host != cf.host {
                    bail!("remote host {} != local host {}", p.host, cf.host);
                }
                println!("ok, host \"{}\"", p.host);
                return Ok(());
            }
            /*
             * XXX progenitor needs a real error type
             *
            Err(Error::ResponseError(e)) => {
                let status = e.status.as_u16();
                if status == 403 || status == 401 {
                    if !authfail_report {
                        eprintln!("auth failure; waiting for approval");
                        authfail_report = true;
                    }
                } else if status >= 400 && status <= 499 {
                    bail!("request error; giving up: {:?}", e);
                } else {
                    eprintln!("request error; retrying: {:?}", e);
                }
            }
             * XXX
             */
            Err(e) => {
                eprintln!("other error; retrying: {:?}", e);
            }
        }

        sleep_ms(5000);
    }
}

async fn cmd_exec(l: Level<()>) -> Result<()> {
    exec_common(l, false).await
}

async fn cmd_cron(l: Level<()>) -> Result<()> {
    /*
     * If we are using the "cron" variant, don't emit error messages to stderr
     * for transient issues as this will result in the cron mail we are
     * generally trying to avoid!
     */
    exec_common(l, true).await
}

async fn exec_common(mut l: Level<()>, silent: bool) -> Result<()> {
    l.usage_args(Some("JOBNAME SCRIPT..."));
    let a = args!(l);

    if a.args().len() < 1 {
        bad_args!(l, "specify a job name");
    }

    let job = a.args()[0].to_string();
    let script = a
        .args()
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if script.is_empty() {
        bail!("no script?");
    }

    let lc = load_config()?;
    let cf = lc
        .config
        .as_ref()
        .ok_or_else(|| anyhow!("no configuration file; enrol first"))?;
    let c = make_client(cf)?;

    let id = ReportId::builder()
        .host(&cf.host)
        .job(&job)
        .uuid(genkey(32))
        .pid(std::process::id())
        .time(Utc::now());

    let start_time = Utc::now();
    let rx = exec::run(&["/usr/bin/bash", "-c", &script])?;

    /*
     * Report that the job has started to the server:
     */
    let body = ReportStartBody::builder()
        .id(id.clone())
        .script(&script)
        .start_time(start_time);

    loop {
        if let Err(e) = c.report_start().body(body.clone()).send().await {
            if !silent {
                println!("ERROR: {:?}", e);
            }
            sleep_ms(1000);
            continue;
        }
        break;
    }

    loop {
        match rx.recv()? {
            Activity::Output(o) => loop {
                let res = c
                    .report_output()
                    .body_map(|b| b.id(id.clone()).record(o.to_record()))
                    .send()
                    .await;
                if let Err(e) = res {
                    if !silent {
                        println!("ERROR: {:?}", e);
                    }
                    sleep_ms(1000);
                    continue;
                }
                break;
            },
            Activity::Exit(ed) => loop {
                let res = c
                    .report_finish()
                    .body_map(|b| {
                        b.id(id.clone())
                            .duration_millis(ed.duration_ms)
                            .end_time(ed.when)
                            .exit_status(ed.code)
                    })
                    .send()
                    .await;
                if let Err(e) = res {
                    if !silent {
                        println!("ERROR: {:?}", e);
                    }
                    sleep_ms(1000);
                    continue;
                }
                break;
            },
            Activity::Complete => {
                break Ok(());
            }
        }
    }
}
