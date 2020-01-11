use crate::config::Config;
use chrono::prelude::*;

use notify_rust::Notification;
use serde_json::from_str;
use webbrowser;

#[macro_use]
extern crate lazy_static;

mod config;
mod utils;

use crate::config::{Post, RSS};

fn daemon_runtime(mut config: Config) -> std::io::Result<()> {
    loop {
        for feed in &config.rss_feeds {
            let previous_date = config
                .feeds_date
                .entry(String::from(feed))
                .or_insert_with(|| {
                    FixedOffset::east(0).from_utc_datetime(&NaiveDateTime::from_timestamp(0, 0))
                });
            let response = reqwest::blocking::get(feed).expect("Request failed");
            let rss_feed: RSS = from_str(&response.text().unwrap()).unwrap();
            let mut max_date = *previous_date;

            for item in rss_feed.channel.item {
                match DateTime::parse_from_rfc2822(&item.pub_date) {
                    Ok(date) => {
                        if &date <= previous_date {
                            continue;
                        }
                        if date > max_date {
                            max_date = date;
                        }
                    }
                    Err(_) => continue,
                }
                let post = Post::from(item);

                let mut notification = Notification::from(&post);

                std::thread::spawn(move || {
                    notification
                        .timeout(3)
                        .show()
                        .unwrap()
                        .wait_for_action(|action| if let "default" = action {
                                if webbrowser::open(&post.link).is_err() {
                                    eprintln!("Could not open link {}", &post.link);
                                }
                        })
                });
            }
            (*config
                .feeds_date
                .entry(String::from(feed))
                .or_insert(max_date)) = max_date;
            println!("{:?}", config.feeds_date);
            std::fs::write(
                &config.dates_file_path,
                serde_json::to_string(&config.feeds_date).unwrap(),
            )?;
        }
        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}

fn main() -> std::io::Result<()> {
    let config = config::setup()?;
    let daemon = config::daemon()?;

    if config.rss_feeds.is_empty() {
        eprintln!("No feeds found.");
        std::process::exit(1);
    }

    match daemon.start() {
        Ok(_) => daemon_runtime(config)?,
        Err(e) => eprintln!("Error, {}", e),
    };
    Ok(())
}
