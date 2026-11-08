// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_session_protocol::hostlink::LinkKey;

fn desktop() -> Pinned {
    Pinned {
        device: "01JB0DESKTOP00000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

fn start(profile: &str) -> session::SignInToStart {
    session::SignInToStart {
        profile: profile.into(),
        provider: None,
        by: session::SignInBy::Driven,
    }
}

/// A runner whose sign-in of `work` is `script`, for its desktop alone, and none of another
/// profile.
fn runs(script: &'static str) -> SigningIn {
    Arc::new(move |_desktop, start| {
        if start.profile != "work" {
            return Err(format!(
                "this runner keeps no profile {} for you",
                start.profile
            ));
        }
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", script]);
        Ok(command)
    })
}

/// Reads sign-in `sign_in` until it ended: what it printed on each output, and how it ended.
async fn read_to_its_end(signs: &SignIns, sign_in: u32) -> (String, String, Option<i32>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    for _ in 0..200 {
        match signs.next(sign_in).await.expect("read") {
            session::Answer::SignInPrinted {
                out: more_out,
                err: more_err,
                ended,
            } => {
                out.extend(more_out);
                err.extend(more_err);
                if let Some(ended) = ended {
                    return (
                        String::from_utf8_lossy(&out).into_owned(),
                        String::from_utf8_lossy(&err).into_owned(),
                        ended.code,
                    );
                }
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("it never ended");
}

fn number(answer: session::Answer) -> u32 {
    match answer {
        session::Answer::SignInStarted { sign_in } => sign_in,
        other => panic!("{other:?}"),
    }
}

/// **A sign-in over the link prints to the desktop, takes what it types, and says how it ended**,
/// each output apart, as a `kubectl exec -i` carried them.
#[tokio::test]
async fn a_sign_in_over_the_link_is_read_typed_to_and_ends_with_its_status() {
    let signs = SignIns::default();
    let sign_in = number(
        signs
            .start(
                &runs("echo 'Paste code:'; echo note >&2; read code; echo \"got $code\"; exit 3"),
                &desktop(),
                start("work"),
            )
            .await
            .expect("started"),
    );

    signs
        .input(sign_in, b"abc#state\n".to_vec())
        .await
        .expect("typed");
    let (out, err, code) = read_to_its_end(&signs, sign_in).await;

    assert_eq!(out, "Paste code:\ngot abc#state\n");
    assert_eq!(err, "note\n");
    assert_eq!(code, Some(3));
    assert!(signs.next(sign_in).await.is_err(), "let go once it ended");
}

/// **What the runner refuses to run is not started**, and a sign-in another link ran is none of
/// this one's.
#[tokio::test]
async fn a_profile_the_runner_does_not_keep_for_the_desktop_is_not_signed_in() {
    let signs = SignIns::default();

    let refused = signs
        .start(&runs("echo nothing"), &desktop(), start("other"))
        .await
        .unwrap_err();

    assert_eq!(refused.code, session::NOT_STARTED);
    assert!(refused.why.contains("keeps no profile"), "{}", refused.why);
    assert!(signs.next(1).await.is_err());
    assert!(signs.input(1, b"x".to_vec()).await.is_err());
}

/// **An ended sign-in's input ends**, so a login held to its input stops; and every sign-in a
/// link ran ends with the link.
#[tokio::test]
async fn a_sign_in_ends_with_its_input_and_with_its_link() {
    let signs = SignIns::default();
    let ended = number(
        signs
            .start(&runs("cat; exit 0"), &desktop(), start("work"))
            .await
            .expect("started"),
    );
    let held = signs.running(ended).expect("running");
    assert!(matches!(signs.end(ended), session::Answer::Done));
    assert!(signs.input(ended, b"late".to_vec()).await.is_err());
    let mut waited = 0;
    while held.child.lock().unwrap().try_wait().unwrap().is_none() {
        assert!(waited < 200, "it ended with its input");
        waited += 1;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let signs = SignIns::default();
    let left = number(
        signs
            .start(&runs("exec sleep 600"), &desktop(), start("work"))
            .await
            .expect("started"),
    );
    let held = signs.running(left).expect("running");
    drop(signs);
    let mut waited = 0;
    while held.child.lock().unwrap().try_wait().unwrap().is_none() {
        assert!(waited < 400, "it ended with its link");
        waited += 1;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
