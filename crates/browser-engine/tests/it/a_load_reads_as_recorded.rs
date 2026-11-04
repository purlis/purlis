// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What two kinds of load read as on a real engine. Temporary.

#![cfg(unix)]

use crate::common;

use common::drive::{call, sees, snapshot};
use common::site::started;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::Call;

#[test]
fn a_status_is_read_as_a_page() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let address = WebAddress::parse("http://fixture.test/answers-refused?v=a%40b").unwrap();
    call(&browser, Call::Open { address });
    assert!(
        sees(&browser, "answered refused"),
        "{}",
        snapshot(&browser).len()
    );
}

#[test]
fn a_failed_tunnel_reads_its_query() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let address = WebAddress::parse("https://fixture.test/x?v=a%40b").unwrap();
    let _ = browser.call(&common::drive::next_id(), Call::Open { address });
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert!(snapshot(&browser).contains("a%40b"));
}
