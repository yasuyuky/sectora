use std::env;
use std::str::FromStr;

pub fn init(appname: Option<&str>) {
    let log_level_env = env::var("LOG_LEVEL").unwrap_or(String::from("OFF"));
    let log_level = log::LevelFilter::from_str(&log_level_env).unwrap_or(log::LevelFilter::Off);
    syslog::init(syslog::Facility::LOG_AUTH, log_level, appname).unwrap_or_default();
}
