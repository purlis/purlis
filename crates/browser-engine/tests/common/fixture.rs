// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A pretend Chrome for Testing: an archive laid out as the real one is, a manifest pinning
//! it, and a download that serves bytes from memory and counts what it was asked for.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Cursor, Read, Write};

use chrono::{DateTime, Utc};
use purlis_browser_engine::install::{Fetch, Manifest};
use sha2::{Digest, Sha256};

pub const VERSION: &str = "155.0.8059.79";

/// A zip with `chrome-linux64/chrome`, executable, as Chrome for Testing's linux64 archive.
pub fn archive(marker: &str) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let exe = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        zip.add_directory("chrome-linux64/", exe).unwrap();
        zip.start_file("chrome-linux64/chrome", exe).unwrap();
        zip.write_all(format!("#!/bin/sh\necho {marker}\n").as_bytes())
            .unwrap();
        zip.start_file("chrome-linux64/resources.pak", exe.unix_permissions(0o644))
            .unwrap();
        zip.write_all(b"resources").unwrap();
        zip.finish().unwrap();
    }
    out.into_inner()
}

/// [`archive`], with one more entry: a link at `name` to `target`.
pub fn archive_with_link(name: &str, target: &str) -> Vec<u8> {
    let mut out = Cursor::new(archive("pinned"));
    {
        let mut zip = zip::ZipWriter::new_append(&mut out).unwrap();
        zip.add_symlink(name, target, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap();
    }
    out.into_inner()
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn url(version: &str) -> String {
    format!(
        "https://storage.googleapis.com/chrome-for-testing-public/{version}/linux64/chrome-linux64.zip"
    )
}

/// The manifest's text for `version`, issued at `issued`, pinning `digest`.
pub fn manifest_text(version: &str, issued: &str, digest: &str) -> String {
    format!(
        r#"{{"version":"{version}","issued":"{issued}","downloads":[{{"platform":"linux64","url":"{url}","sha256":"{digest}"}}]}}"#,
        url = url(version)
    )
}

pub fn manifest(version: &str, issued: &str, digest: &str) -> Manifest {
    Manifest::parse(manifest_text(version, issued, digest).as_bytes()).unwrap()
}

pub fn at(time: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&Utc)
}

/// Serves `files` from memory and remembers every address asked for.
#[derive(Default)]
pub struct Served {
    pub files: HashMap<String, Vec<u8>>,
    pub asked: RefCell<Vec<String>>,
}

impl Served {
    pub fn with(url: String, bytes: Vec<u8>) -> Self {
        let mut served = Self::default();
        served.files.insert(url, bytes);
        served
    }
}

impl Fetch for Served {
    fn fetch(&self, url: &str) -> io::Result<Box<dyn Read + '_>> {
        self.asked.borrow_mut().push(url.to_owned());
        match self.files.get(url) {
            Some(bytes) => Ok(Box::new(Cursor::new(bytes.clone()))),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "404")),
        }
    }
}

/// The engine at `program`, as the person's own choice in Settings: how a test that runs a
/// given executable gets the [`Ready`] every start needs. `program` must exist.
pub fn chosen(program: &std::path::Path) -> purlis_browser_engine::install::Ready {
    use purlis_browser_engine::install::{
        EngineChoice, EngineSettings, Engines, NoSigner, Platform,
    };
    let settings = EngineSettings {
        engine: EngineChoice::Machine(program.to_owned()),
        ..EngineSettings::default()
    };
    Engines::new(
        program,
        Platform::Linux64,
        Manifest::shipped(),
        Box::new(NoSigner),
    )
    .prepare(&settings, &Served::default(), at("2026-10-15T00:00:00Z"))
    .unwrap()
}
