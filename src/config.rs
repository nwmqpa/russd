use crate::utils;
use chrono::prelude::*;
use daemonize::Daemonize;
use notify_rust::{Notification, Timeout};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

lazy_static! {
    pub static ref TMP_DIR: TempDir = tempdir().expect("Cannot create temp dir");
    pub static ref CFG_DIR: PathBuf = directories::BaseDirs::new()
        .unwrap()
        .config_dir()
        .join("russd");
}

pub struct Config {
    pub rss_feeds: Vec<String>,
    pub dates_file_path: PathBuf,
    pub feeds_date: HashMap<String, DateTime<FixedOffset>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RSSItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RSSChannel {
    pub title: String,
    pub link: String,
    pub item: Vec<RSSItem>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RSS {
    pub channel: RSSChannel,
}

#[derive(Debug)]
pub struct Post {
    pub title: String,
    pub description: String,
    pub link: String,
    pub icon: PathBuf,
}

impl From<&Post> for Notification {
    fn from(post: &Post) -> Notification {
        Notification::new()
            .summary(&post.title)
            .body(&post.description)
            .action("default", "default")
            .timeout(Timeout::Milliseconds(0))
            .icon(&format!(
                "{}{}",
                "file://",
                post.icon.to_str().expect("Cannot convert to str")
            ))
            .finalize()
    }
}

impl From<RSSItem> for Post {
    fn from(item: RSSItem) -> Post {
        Post {
            title: String::from(&item.title),
            description: String::from(&item.description),
            link: String::from(&item.link),
            icon: utils::get_icon(&item.link),
        }
    }
}

pub fn setup() -> Result<Config, std::io::Error> {
    let config_file_path = CFG_DIR.join("russd.conf");
    let dates_file_path = CFG_DIR.join("dates.json");

    if !CFG_DIR.exists() {
        std::fs::create_dir(CFG_DIR.as_os_str())?;
    }

    if !Path::new(&config_file_path).exists() {
        File::create(config_file_path)?;
    }

    let feeds_date = if !Path::new(&dates_file_path).exists() {
        let feeds_date = HashMap::new();
        std::fs::write(
            &dates_file_path,
            serde_json::to_string(&feeds_date).unwrap(),
        )?;
        feeds_date
    } else {
        let file = std::fs::read_to_string(&dates_file_path)?;
        serde_json::from_str(&file).unwrap()
    };

    let file = std::fs::read_to_string(CFG_DIR.join("russd.conf"))?;
    let rss_feeds = file
        .lines()
        .filter(|x| !x.is_empty())
        .map(String::from)
        .collect();
    Ok(Config {
        rss_feeds,
        feeds_date,
        dates_file_path,
    })
}

pub fn daemon() -> Result<Daemonize<&'static str>, std::io::Error> {
    let stdout = File::create(TMP_DIR.path().join("daemon.out"))?;
    let stderr = File::create(TMP_DIR.path().join("daemon.err"))?;

    Ok(Daemonize::new()
        .pid_file("/tmp/russd.pid")
        .working_directory(TMP_DIR.path().as_os_str())
        .stdout(stdout)
        .stderr(stderr)
        .exit_action(|| println!("Running in the background"))
        .privileged_action(|| "Has not dropped priviliges yet"))
}
