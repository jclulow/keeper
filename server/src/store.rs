use anyhow::{bail, Context, Result};
use chrono::prelude::*;
use keeper_common::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use slog::{debug, error, info, warn, Logger};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct KeyFile {
    pub host: String,
    pub key: String,
    pub time_create: DateTime<Utc>,
    #[serde(default)]
    pub global_view: bool,
}

/**
 * Host and job names must be safe as a filename, as we will store the
 * associated user account in "keys/<hostname>.json" and jobs are stored as
 * "reports/<hostname>/<jobname>/...".
 */
pub fn name_ok(n: &str) -> bool {
    n.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
    }) && n.len() >= 2
}

/**
 * Clients submit their own key during enrolment, and it will be included in
 * "Authorization" headers later.  Make sure the format is at least plausible.
 */
pub fn key_ok(k: &str) -> bool {
    k.chars().all(|c| c.is_ascii_alphanumeric()) && k.len() == 64
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
pub struct ReportSummary {
    pub host: String,
    pub job: String,
    pub when: DateTime<Utc>,
    pub status: i32,
    pub duration_seconds: i32,
    pub age_seconds: i32,
}

pub struct ReportStore {
    dir: PathBuf,
    log: Logger,
}

impl ReportStore {
    pub fn new<P: AsRef<Path>>(log: Logger, dir: P) -> Result<ReportStore> {
        Ok(ReportStore {
            log,
            dir: dir.as_ref().to_path_buf(),
        })
    }

    fn list_reports(
        &self,
        host: &str,
        job: &str,
        year: u32,
        month: u32,
        day: u32,
    ) -> Result<Vec<i64>> {
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

    fn list_days(
        &self,
        host: &str,
        job: &str,
        year: u32,
        month: u32,
    ) -> Result<Vec<u32>> {
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

    fn list_months(
        &self,
        host: &str,
        job: &str,
        year: u32,
    ) -> Result<Vec<u32>> {
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

    fn list_years(&self, host: &str, job: &str) -> Result<Vec<u32>> {
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

    fn list_jobs(&self, host: &str) -> Result<Vec<String>> {
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

    fn list_hosts(&self) -> Result<Vec<String>> {
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

    fn reportpath(
        &self,
        host: &str,
        job: &str,
        time: &DateTime<Utc>,
    ) -> Result<PathBuf> {
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

    pub fn summary(&self, perjob: usize) -> Result<Vec<ReportSummary>> {
        let mut out = Vec::new();

        for host in self.list_hosts()?.iter() {
            'job: for job in self.list_jobs(host)?.iter() {
                let mut c = 0usize;
                for y in self.list_years(host, job)?.iter() {
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

                                let dt = Utc.timestamp_millis_opt(*r).unwrap();
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

    pub fn load(
        &self,
        host: &str,
        job: &str,
        time: &DateTime<Utc>,
    ) -> Result<Option<PostFile>> {
        let targ = self.reportpath(host, job, time)?;
        load_file(&targ)
    }

    pub fn store(
        &self,
        host: &str,
        job: &str,
        time: &DateTime<Utc>,
        post: &PostFile,
    ) -> Result<()> {
        let targ = self.reportpath(host, job, time)?;
        store_file(&targ, post, false)
    }
}

pub struct KeyStore {
    dir: PathBuf,
    log: Logger,
}

impl KeyStore {
    pub fn new<P: AsRef<Path>>(log: Logger, dir: P) -> Result<KeyStore> {
        Ok(KeyStore {
            log,
            dir: dir.as_ref().to_path_buf(),
        })
    }

    fn keypath(&self, set: &str, host: Option<&str>) -> Result<PathBuf> {
        if let Some(host) = host {
            if !name_ok(host) {
                bail!("invalid hostname");
            }
        }

        let mut kpath = self.dir.clone();
        kpath.push(set);
        std::fs::create_dir_all(&kpath)?;
        if let Some(host) = host {
            kpath.push(&format!("{}.json", host));
        }

        Ok(kpath)
    }

    pub fn check_key(&self, key: &str) -> Result<Option<Auth>> {
        let kdir = self.keypath("keys", None)?;

        let mut dir = std::fs::read_dir(&kdir)?;
        let mut out: Option<Auth> = None;
        while let Some(ent) = dir.next().transpose()? {
            if !ent.file_type()?.is_file() {
                continue;
            }

            let kpath = ent.path();

            if let Ok(Some(f)) = load_file::<KeyFile>(&kpath) {
                if key == f.key {
                    if let Some(out) = out {
                        bail!("duplicate keys? {} and {}", f.host, out.host);
                    } else {
                        out = Some(Auth {
                            host: f.host.to_string(),
                            global_view: f.global_view,
                        });
                    }
                }
            }
        }

        Ok(out)
    }

    pub fn enrol_key(&self, host: &str, key: &str) -> Result<bool> {
        if !name_ok(host) || !key_ok(key) {
            return Ok(false);
        }

        /*
         * If we have already confirmed this host, let's log a warning but
         * pretend to the client that everything was OK.
         *
         * This routine is run by the API layer with the rwlock held for write,
         * so multiple requests should not race here.
         */
        match std::fs::metadata(self.keypath("keys", Some(host))?) {
            Ok(f) if f.is_file() => {
                warn!(
                    self.log,
                    "re-enrolment for already confirmed host {}", host
                );
                return Ok(true);
            }
            _ => {}
        }

        let kpath = self.keypath("enrol", Some(host))?;

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
            .open(kpath);
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
        bw.write_all(&buf)?;
        bw.flush()?;
        Ok(true)
    }
}

#[derive(Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct OutputRecord {
    pub time: DateTime<Utc>,
    pub stream: String,
    pub msg: String,
}

#[derive(Serialize, Deserialize)]
pub struct PostFile {
    pub report_pid: u32,
    pub report_uuid: String,
    pub report_time: DateTime<Utc>,
    pub time_start: DateTime<Utc>,
    pub time_end: Option<DateTime<Utc>>,
    pub script: String,
    pub duration: Option<u64>,
    pub status: Option<i32>,
    #[serde(default)]
    pub output: Vec<OutputRecord>,
    #[serde(default)]
    pub sealed: bool,
}

impl PostFile {
    pub fn duration_seconds(&self) -> i32 {
        u64ton(self.duration.unwrap() / 1000)
    }
}

pub struct Auth {
    pub host: String,
    pub global_view: bool,
}
