use keeper_openapi::apis::configuration::Configuration;
use keeper_openapi::apis::default_api::*;
use keeper_openapi::models::*;
use keeper_openapi::apis::Error;
use anyhow::{Result, anyhow, bail};
use chrono::prelude::*;
use reqwest::{
    ClientBuilder,
    header::HeaderMap,
    header::HeaderValue,
    header::AUTHORIZATION,
};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use keeper_common::*;

mod exec;
use exec::Activity;

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    baseurl: String,
    host: String,
    key: String,
}

fn configure(cf: Option<&ConfigFile>) -> Result<Configuration> {
    let cf = if let Some(cf) = cf {
        cf
    } else {
        bail!("no configuration file; enrol first");
    };

    let ah = format!("Bearer {}", cf.key);

    let mut defhdr = HeaderMap::new();
    defhdr.insert(AUTHORIZATION, HeaderValue::from_str(&ah)?);

    let client = ClientBuilder::new()
        .default_headers(defhdr)
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(30))
        .build()?;

    Ok(Configuration {
        base_path: cf.baseurl.to_string(),
        client,
        ..Default::default()
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cmd = std::env::args()
        .nth(1)
        .ok_or(anyhow!("must specify a command"))?;

    let cfgpath = if let Some(mut home) = dirs::home_dir() {
        home.push(".keeper.json");
        home
    } else {
        bail!("could not find home directory");
    };

    let cf = load_file::<ConfigFile>(&cfgpath)?;

    match cmd.as_str() {
        "enrol" => {
            /*
             * Accept intended hostname and base URL for enrolment.
             */
            let host = std::env::args()
                .nth(2)
                .ok_or(anyhow!("specify a host name for enrolment"))?;
            let baseurl = std::env::args()
                .nth(3)
                .ok_or(anyhow!("specify a base URL for enrolment"))?;

            let cf = if let Some(cf) = cf {
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
                store_file(&cfgpath, &cf, true)?;
                cf
            };

            let cfg = configure(Some(&cf))?;

            loop {
                let res = enrol(&cfg, EnrolBody {
                    host: cf.host.to_string(),
                    key: cf.key.to_string(),
                })
                .await;

                match res {
                    Ok(_) => {
                        println!("ok");
                        return Ok(());
                    }
                    Err(Error::ResponseError(e)) => {
                        let status = e.status.as_u16();
                        if status >= 400 && status <= 499 {
                            bail!("request error; giving up: {:?}", e);
                        } else {
                            eprintln!("request error; retrying: {:?}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("other error; retrying: {:?}", e);
                    }
                }

                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        "ping" => {
            let cfg = configure(cf.as_ref())?;
            let cf = cf.unwrap();
            let mut authfail_report = false;

            loop {
                let res = ping(&cfg).await;

                match res {
                    Ok(p) => {
                        if p.host != cf.host {
                            bail!("remote host {} != local host {}", p.host,
                                cf.host);
                        }
                        println!("ok, host \"{}\"", p.host);
                        return Ok(());
                    }
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
                    Err(e) => {
                        eprintln!("other error; retrying: {:?}", e);
                    }
                }

                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }
        "exec" | "cron" => {
            let cfg = configure(cf.as_ref())?;
            let cf = cf.unwrap();

            /*
             * If we are using the "cron" variant, don't emit error messages to
             * stderr for transient issues as this will result in the cron mail
             * we are generally trying to avoid!
             */
            let silent = cmd.as_str() == "cron";

            let job = std::env::args()
                .nth(2)
                .ok_or(anyhow!("specify a job name"))?;
            let script = std::env::args().skip(3).collect::<Vec<_>>().join(" ");
            if script.is_empty() {
                bail!("no script?");
            }

            let id = ReportId {
                host: cf.host.clone(),
                job,
                uuid: genkey(32),
                pid: std::process::id() as i32,
                time: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            };

            let start_time = Utc::now().to_string();
            let rx = exec::run(&["/usr/bin/bash", "-c", &script])?;

            /*
             * Report that the job has started to the server:
             */
            loop {
                let res = report_start(&cfg, ReportStartBody {
                    id: id.clone(),
                    start_time: start_time.clone(),
                    script: script.clone(),
                })
                .await;
                if let Err(e) = res {
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
                        let res = report_output(&cfg, ReportOutputBody {
                            id: id.clone(),
                            record: o.to_record(),
                        })
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
                        let res = report_finish(&cfg, ReportFinishBody {
                            id: id.clone(),
                            duration_millis: ed.duration_ms as i32,
                            end_time: ed.when.to_string(),
                            exit_status: ed.code,
                        })
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
                        break;
                    }
                }
            }
        }
        x => {
            eprintln!("unrecognised command: {}", x);
        }
    }

    Ok(())
}
