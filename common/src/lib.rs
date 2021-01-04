use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::io::{Read, Write, BufReader, BufWriter, ErrorKind};
use std::path::Path;
use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

pub fn load_file<T>(p: &Path) -> Result<Option<T>>
where
    for<'de> T: Deserialize<'de>,
{
    let f = match File::open(p) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => bail!("load file {}: {:?}", p.display(), e),
        Ok(f) => f,
    };
    let mut br = BufReader::new(f);
    let mut buf: Vec<u8> = Vec::new();
    br.read_to_end(&mut buf)?;
    Ok(serde_json::from_slice(buf.as_slice())?)
}

pub fn store_file<T>(p: &Path, data: &T, private: bool) -> Result<()>
where
    T: Serialize,
{
    let mut tmp = p.to_path_buf();
    let mut n = p.file_name().unwrap().to_os_string();
    n.push(".tmp");
    tmp.set_file_name(n);

    let mut f = OpenOptions::new();
    f.create_new(true);
    f.write(true);
    if private {
        f.mode(0o600);
    }
    let f = f.open(&tmp)?;
    let mut bw = BufWriter::new(f);
    serde_json::to_writer_pretty(&mut bw, data)?;
    bw.flush()?;

    std::fs::rename(&tmp, p)?;
    Ok(())
}

pub fn genkey(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(|c| c as char)
        .collect()
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
