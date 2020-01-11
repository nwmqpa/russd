use crate::config::TMP_DIR;
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};

use website_icon_extract::extract_icons;

pub fn get_icon(link: &str) -> PathBuf {
    let icons = extract_icons(link, "", 2).unwrap_or_else(|_| vec![String::from(
        "https://www.mozilla.org/media/img/favicons/firefox/browser/favicon.f093404c0135.ico",
    )]);
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
        TMP_DIR
            .path()
            .join(icon_link.replace("/", "_").replace("http", ""))
    } else {
        download_file(icon_link)
    }
}

pub fn download_file(link: &str) -> PathBuf {
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
