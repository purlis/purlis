//! A host opens a view or a timeline on a stream of its own, and the client learns which from
//! the stream's first byte, before anything else of it is read.

use bytes::Bytes;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session;
use purlis_session_protocol::stream::{self, Kind};
use purlis_session_protocol::view::{Attacher, Chunk, Limits, ViewId, Viewer};
use tokio::io::{AsyncWriteExt, duplex};

mod common;
use common::HELD;

async fn linked() -> (Link, Link) {
    let (a, b) = duplex(256 * 1024);
    let (client, host) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    (client.unwrap(), host.unwrap())
}

#[tokio::test]
async fn a_view_is_told_by_its_first_byte_and_still_reads_whole() {
    let (mut client, host) = linked().await;
    let _feed = Attacher::new(host.opener(), Limits::default())
        .attach(ViewId(4), Bytes::from_static(b"snap"))
        .await
        .unwrap();
    let opened = stream::take(client.accept().await.unwrap()).await.unwrap();
    assert_eq!(opened.kind(), Kind::View);
    let mut reader = Viewer::default().accept_opened(opened).await.unwrap();
    assert_eq!(reader.view(), ViewId(4));
    let chunk = reader.next().await.unwrap().unwrap();
    assert_eq!(chunk, Chunk::Snapshot(Bytes::from_static(b"snap")));
}

#[tokio::test]
async fn a_stream_of_an_unknown_kind_is_refused_by_name() {
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        s.write_all(&[0x7f, 0, 0, 0, 1]).await.unwrap();
        s.flush().await.unwrap();
        // Hold it open: the client refuses on the byte, not on the close.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(s);
    });
    let refused = stream::take(client.accept().await.unwrap())
        .await
        .unwrap_err();
    let text = refused.to_string();
    assert!(text.contains("0x7f"), "{text}");
}

#[tokio::test]
async fn a_timeline_byte_is_told_apart_from_a_view() {
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        s.write_all(&[purlis_session_protocol::timeline::TIMELINE])
            .await
            .unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(s);
    });
    let opened = stream::take(client.accept().await.unwrap()).await.unwrap();
    assert_eq!(opened.kind(), Kind::Timeline);
    // Handed to the viewer anyway, it is refused rather than read as a view.
    let refused = match Viewer::default().accept_opened(opened).await {
        Ok(_) => panic!("a timeline stream was read as a view"),
        Err(e) => e,
    };
    assert!(refused.to_string().contains("not a view"), "{refused}");
}
