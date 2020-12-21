use chrono::prelude::*;
use super::OutputRecord;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::process::{Command, Stdio};
use std::os::unix::process::ExitStatusExt;
use std::io::{Read, BufReader, BufRead};
use std::ffi::OsStr;
use std::time::SystemTime;
use anyhow::Result;

fn spawn_reader<T>(tx: Sender<Activity>, name: &str, stream: Option<T>)
    -> Option<std::thread::JoinHandle<()>>
where
    T: Read + Send + 'static,
{
    let name = name.to_string();
    let stream = match stream {
        Some(stream) => stream,
        None => return None,
    };

    Some(std::thread::spawn(move || {
        let mut r = BufReader::new(stream);

        loop {
            let mut buf: Vec<u8> = Vec::new();

            /*
             * We have no particular control over the output from the child
             * processes we run, so we read until a newline character without
             * relying on totally valid UTF-8 output.
             */
            match r.read_until(b'\n', &mut buf) {
                Ok(0) => {
                    /*
                     * EOF.
                     */
                    return;
                }
                Ok(_) => {
                    let s = String::from_utf8_lossy(&buf);

                    tx.send(Activity::Output(OutputRecord {
                        msg: s.trim_end().to_string(),
                        stream: name.to_string(),
                        time: Utc::now().to_string(),
                    })).unwrap();
                }
                Err(e) => {
                    /*
                     * Try to report whatever error we experienced to the
                     * server:
                     */
                    tx.send(Activity::Output(OutputRecord {
                        msg: format!("failed to read {}: {:?}", name, e),
                        stream: "error".to_string(),
                        time: Utc::now().to_string(),
                    })).unwrap();
                    return;
                }
            }
        }
    }))
}

pub struct ExitDetails {
    pub duration_ms: u64,
    pub when: DateTime<Utc>,
    pub code: i32,
}

pub enum Activity {
    Output(OutputRecord),
    Exit(ExitDetails),
    Complete,
}

impl Activity {
    fn exit(start: &SystemTime, end: &SystemTime, code: i32)
        -> Activity
    {
        Activity::Exit(ExitDetails {
            duration_ms: end.duration_since(*start).unwrap().as_millis() as u64,
            when: Utc::now(),
            code
        })
    }
}

pub fn run<S: AsRef<OsStr>>(args: &[S]) -> Result<Receiver<Activity>> {
    let args: Vec<&OsStr> = args.iter().map(|s| s.as_ref()).collect();

    let (tx, rx) = channel::<Activity>();

    let mut cmd = Command::new(&args[0]);

    if args.len() > 1 {
        cmd.args(&args[1..]);
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let start = SystemTime::now();
    let mut child = cmd.spawn()?;

    let readout = spawn_reader(tx.clone(), "stdout", child.stdout.take());
    let readerr = spawn_reader(tx.clone(), "stderr", child.stderr.take());

    std::thread::spawn(move || {
        if let Some(t) = readout {
            t.join().expect("join stdout thread");
        }
        if let Some(t) = readerr {
            t.join().expect("join stderr thread");
        }

        let wait = child.wait();
        let end = SystemTime::now();
        match wait {
            Err(e) => {
                tx.send(Activity::Output(OutputRecord {
                    msg: format!("child wait error: {:?}", e),
                    stream: "error".to_string(),
                    time: Utc::now().to_string(),
                })).unwrap();
                tx.send(Activity::exit(&start, &end, std::i32::MAX)).unwrap();
            }
            Ok(es) => {
                if let Some(sig) = es.signal() {
                    tx.send(Activity::Output(OutputRecord {
                        msg: format!("child terminated by signal {}", sig),
                        stream: "error".to_string(),
                        time: Utc::now().to_string(),
                    })).unwrap();
                }
                let code = if let Some(code) = es.code() {
                    code
                } else {
                    std::i32::MAX
                };
                tx.send(Activity::exit(&start, &end, code)).unwrap();
            }
        }

        tx.send(Activity::Complete).unwrap();
    });

    Ok(rx)
}
