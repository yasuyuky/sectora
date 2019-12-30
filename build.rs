use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

struct Ignore;

impl<E> From<E> for Ignore where E: Error
{
    fn from(_: E) -> Ignore { Ignore }
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out_dir.join("commit-info.txt")).unwrap()
                                                 .write_all(commit_info().as_bytes())
                                                 .unwrap();
    let log_level = env::var("LOG_LEVEL").unwrap_or(String::from("OFF"));
    File::create(out_dir.join("log-level.txt")).unwrap()
                                               .write_all(log_level.as_bytes())
                                               .unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}

fn commit_info() -> String {
    match (commit_date(), commit_hash()) {
        (Ok(date), Ok(hash)) => format!(" {} {}", date.trim_end(), hash.trim_end(),),
        _ => String::new(),
    }
}

fn commit_hash() -> Result<String, Ignore> {
    Ok(String::from_utf8(Command::new("git").args(&["rev-parse",
                                                    "--short=10",
                                                    "HEAD"])
                                            .output()?
                                            .stdout)?)
}

fn commit_date() -> Result<String, Ignore> {
    Ok(String::from_utf8(Command::new("git").args(&["log",
                                                    "-1",
                                                    "--date=short",
                                                    "--pretty=format:%cd"])
                                            .output()?
                                            .stdout)?)
}
