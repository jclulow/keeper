use anyhow::{anyhow, bail, Result};
use chrono::prelude::*;
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

fn make_client(cf: Option<&ConfigFile>) -> Result<Client> {
    let cf = if let Some(cf) = cf {
        cf
    } else {
        bail!("no configuration file; enrol first");
    };

    Ok(keeper_openapi::ClientBuilder::new(&cf.baseurl)
        .bearer_token(&cf.key)
        .build()?)
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

            let c = make_client(Some(&cf))?;

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
        "ping" => {
            let c = make_client(cf.as_ref())?;
            let cf = cf.unwrap();
            /* XXX let mut authfail_report = false; */

            loop {
                match c.ping().send().await {
                    Ok(p) => {
                        if p.host != cf.host {
                            bail!(
                                "remote host {} != local host {}",
                                p.host,
                                cf.host
                            );
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
        "exec" | "cron" => {
            let c = make_client(cf.as_ref())?;
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
                if let Err(e) = c.report_start().body(body.clone()).send().await
                {
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
                            .body_map(|b| {
                                b.id(id.clone()).record(o.to_record())
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
