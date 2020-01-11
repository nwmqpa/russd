use chrono::prelude::*;
use notify_rust::Notification;

use webbrowser;

use serde_json::from_str;

#[macro_use]
extern crate lazy_static;

mod config;
mod utils;

use crate::config::{Post, RSS};

fn main() -> std::io::Result<()> {
    let (feeds, mut feeds_date, dates_file_path) = config::setup()?;
    let daemon = config::daemon()?;

    if feeds.is_empty() {
        eprintln!("No feeds found.");
        std::process::exit(1);
    }

    match daemon.start() {
        Ok(_) => loop {
            for feed in &feeds {
                let previous_date = feeds_date.entry(String::from(feed)).or_insert(
                    FixedOffset::east(0).from_utc_datetime(&NaiveDateTime::from_timestamp(0, 0)),
                );
                let response = reqwest::blocking::get(feed).expect("Request failed");
                let rss_feed: RSS = from_str(&response.text().unwrap()).unwrap();
                let mut max_date = previous_date.clone();

                for item in rss_feed.channel.item {
                    match DateTime::parse_from_rfc2822(&item.pubDate) {
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
                            .wait_for_action(|action| match action {
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
                std::fs::write(
                    &dates_file_path,
                    serde_json::to_string(&feeds_date).unwrap(),
                )?;
            }
            std::thread::sleep(std::time::Duration::from_secs(20));
        },
        Err(e) => eprintln!("Error, {}", e),
    };
    Ok(())
}
