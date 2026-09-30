//! Share the Python/SQLite control plane without exposing request bodies or keys.
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};
pub fn call(action: &str, data: Value) -> String {
    let result = (|| -> Result<String, ()> {
        let run = std::env::var("OPENBRIDGE_PROBE_RUN").map_err(|_| ())?;
        let mut child = Command::new("python3")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/probe.py"))
            .args(["control", &run, action])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ())?;
        let mut stdin = child.stdin.take().ok_or(())?;
        stdin
            .write_all(&serde_json::to_vec(&data).map_err(|_| ())?)
            .map_err(|_| ())?;
        drop(stdin);
        let output = child.wait_with_output().map_err(|_| ())?;
        if !output.status.success() || output.stdout.len() > 256 {
            return Err(());
        }
        String::from_utf8(output.stdout)
            .map(|text| text.trim().to_owned())
            .map_err(|_| ())
    })();
    result.unwrap_or_else(|_| {
        eprintln!("shared probe ledger rejected operation; no retry");
        std::process::exit(2)
    })
}
