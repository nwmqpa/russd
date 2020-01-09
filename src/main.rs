use std::fs::File;
use std::io::copy;
use std::path::Path;
use std::path::PathBuf;

use chrono::prelude::*;

use daemonize::Daemonize;
use notify_rust::{Notification, Timeout};
use tempfile::{tempdir, TempDir};
use webbrowser;
use website_icon_extract::extract_icons;

use serde::{Deserialize, Serialize};

use serde_xml_rs::from_str;
use std::collections::HashMap;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref TMP_DIR: TempDir = tempdir().expect("Cannot create temp dir");
    static ref CFG_DIR: PathBuf = directories::BaseDirs::new().unwrap().config_dir().join("russd");
}

#[derive(Serialize, Deserialize, Debug)]
struct RSSItem {
    title: String,
    link: String,
    description: String,
    pubDate: String
}

#[derive(Serialize, Deserialize, Debug)]
struct RSSChannel {
    title: String,
    link: String,
    item: Vec<RSSItem>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RSS {
    channel: RSSChannel,
}

#[derive(Debug)]
struct Post {
    title: String,
    description: String,
    link: String,
    icon: PathBuf,
}

impl From<RSSItem> for Post {
    fn from(item: RSSItem) -> Post {
        Post {
            title: String::from(&item.title),
            description: String::from(&item.description),
            link: String::from(&item.link),
            icon: get_icon(&item.link),
        }
    }
}

fn get_icon<'s>(link: &'s str) -> PathBuf {
    let icons = extract_icons(link, "", 2).unwrap_or(vec![String::from("https://www.mozilla.org/media/img/favicons/firefox/browser/favicon.f093404c0135.ico")]);
    let icons = icons
        .into_iter()
        .filter(|link| link.contains("favicon"))
        .collect::<Vec<String>>();
    let icon_link = icons.first().unwrap();

    if Path::new(
        &TMP_DIR
            .path()
            .join(link.replace("/", "_").replace("http", "")),
    )
    .exists()
    {
        return TMP_DIR
            .path()
            .join(icon_link.replace("/", "_").replace("http", ""));
    } else {
        return download_file(icon_link);
    }
}

fn download_file<'s>(link: &'s str) -> PathBuf {
    let mut response = reqwest::blocking::get(link).expect("Request failed");
    let (mut dest, fname) = {
        let fname = TMP_DIR.path().join("tmp.bin");
        (File::create(&fname).expect("Failed to create file"), fname)
    };
    copy(&mut response, &mut dest).expect("Failed to copy");
    let icon = File::open(fname).unwrap();
    let icon_dir = ico::IconDir::read(&icon).unwrap();
    let image = icon_dir.entries()[0].decode().unwrap();
    let new_dest_path = TMP_DIR
        .path()
        .join(link.replace("/", "_").replace("http", ""));
    let mut new_dest = File::create(&new_dest_path).unwrap();
    image.write_png(&mut new_dest).unwrap();
    new_dest_path
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

fn main() -> std::io::Result<()> {
    let stdout = File::create(TMP_DIR.path().join("daemon.out"))?;
    let stderr = File::create(TMP_DIR.path().join("daemon.err"))?;

    let config_file_path = CFG_DIR.join("russd.conf");
    let dates_file_path = CFG_DIR.join("dates.json");

    let mut feeds = Vec::<String>::new();

    if !CFG_DIR.exists() {
        std::fs::create_dir(CFG_DIR.as_os_str())?;
    }

    if !Path::new(&config_file_path).exists() {
        File::create(config_file_path)?;
    }

    let mut feeds_date = if !Path::new(&dates_file_path).exists() {
        let feeds_date = HashMap::<String, DateTime<FixedOffset>>::new();
        std::fs::write(&dates_file_path, serde_json::to_string(&feeds_date).unwrap())?;
        feeds_date

    } else {
        let file = std::fs::read_to_string(&dates_file_path)?;
        serde_json::from_str(&file).unwrap()
    };

    let file = std::fs::read_to_string(CFG_DIR.join("russd.conf"))?;
    let lines = file.lines();

    for line in lines.enumerate() {
        if line.1.len() != 0 {
            feeds.push(String::from(line.1));
        }
    }

    if feeds.len() == 0 {
        eprintln!("No feeds found.");
        std::process::exit(1);
    }

    let daemonize = Daemonize::new()
        .pid_file("/tmp/russd.pid")
        .working_directory(TMP_DIR.path().as_os_str())
        .stdout(stdout)
        .stderr(stderr)
        .exit_action(|| println!("Running in the background"))
        .privileged_action(|| "Has not dropped priviliges yet");

    match daemonize.start() {
        Ok(_) => {
            
            loop {
                for feed in &feeds {
                    let previous_date = feeds_date.entry(String::from(feed)).or_insert(FixedOffset::east(0).from_utc_datetime(&NaiveDateTime::from_timestamp(0, 0)));
                    let response = reqwest::blocking::get(feed).expect("Request failed");
                    let rss_feed: RSS = from_str(&response.text().unwrap()).unwrap();
                    let mut max_date = previous_date.clone();

                    for item in rss_feed.channel.item {
                        match DateTime::parse_from_rfc2822(&item.pubDate) {
                            Ok(date) => {
                                if &date <= previous_date {
                                    continue
                                }
                                if date > max_date {
                                    max_date = date;
                                }
                            },
                            Err(_) => continue
                        }
                        let post = Post::from(item);
                        let notification = Notification::from(&post);
                        
                        std::thread::spawn(move || {
                            notification.show().unwrap().wait_for_action(|action| match action {
                                "default" => {
                                    if !webbrowser::open(&post.link).is_ok() {
                                        println!("Could not open link {}", &post.link);
                                    }
                                }
                                _ => (),
                            })
                        });
                    }
                    (*feeds_date.entry(String::from(feed)).or_insert(max_date)) = max_date;
                    println!("{:?}", feeds_date);
                    std::fs::write(&dates_file_path, serde_json::to_string(&feeds_date).unwrap())?;

                }
                std::thread::sleep(std::time::Duration::from_millis(3000));
            }
        },
        Err(e) => eprintln!("Error, {}", e)
    };
    Ok(())
}
