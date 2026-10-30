// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What Claude Code 2.1.314's `claude auth login` prints with its input a pipe, as its own code
/// writes it: the page, then the prompt for the code, with no line break after it.
const RECORDED: &str = include_str!("claude-2.1.314-login.txt");

fn recorded() -> &'static str {
    RECORDED.trim_end_matches('\n')
}

const PAGE: &str = "https://claude.ai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-\
                    5944d1962f5e&response_type=code&redirect_uri=https%3A%2F%2Fplatform.claude.com\
                    %2Foauth%2Fcode%2Fcallback&scope=user%3Ainference&state=recorded";

fn claude() -> Driven {
    Driven::of("claude", Instant::now()).expect("Claude Code's sign-in is driven")
}

/// A login asking for its code, as the recorded one does.
fn asking() -> Driven {
    let mut driven = claude();
    driven.read(recorded());
    driven
}

/// **Claude Code's own login asks for its page to be opened here, then for the code it shows**.
#[test]
fn claude_codes_login_opens_its_page_here_then_asks_for_the_code() {
    let mut driven = claude();

    assert_eq!(
        driven.read(recorded()),
        [
            Event::Open {
                url: PAGE.to_owned()
            },
            Event::AskCode
        ]
    );
}

/// What a login prints arrives in pieces, cut anywhere: the same events, once each.
#[test]
fn a_login_read_in_pieces_says_each_step_once() {
    let mut driven = claude();
    let mut said = Vec::new();
    for piece in recorded().as_bytes().chunks(7) {
        said.extend(driven.read(&String::from_utf8_lossy(piece)));
    }

    assert_eq!(
        said,
        [
            Event::Open {
                url: PAGE.to_owned()
            },
            Event::AskCode
        ]
    );
}

/// A page wrapped in a terminal's link (OSC 8), as Claude Code prints it to a terminal that
/// shows links, is the same page.
#[test]
fn a_page_in_a_terminals_link_is_the_same_page() {
    let mut driven = claude();
    let said = driven.read(&format!(
        "If the browser didn't open, visit: \u{1b}]8;;{PAGE}\u{7}\u{1b}[94m{PAGE}\u{1b}[39m\u{1b}]8;;\u{7}\n"
    ));

    assert_eq!(
        said,
        [Event::Open {
            url: PAGE.to_owned()
        }]
    );
}

/// **Only the harness's own sign-in pages are opened**, by their host and path exactly: any
/// other page, on any host, its vendor's own among them, and purlis cannot follow the sign-in,
/// and says what it printed.
#[test]
fn a_page_on_another_host_is_never_opened() {
    // A user before the host: written apart, so it reads as no address anywhere.
    let with_a_user = format!("https://someone{}claude.ai/oauth/authorize", '@');
    for page in [
        "https://claude.ai.example.com/oauth/authorize",
        "http://claude.ai/oauth/authorize",
        "https://evilclaude.ai/oauth",
        "file:///etc/passwd",
        with_a_user.as_str(),
        "https://claude.ai:8443/oauth/authorize",
        "https://CLAUDE.AI/oauth/authorize",
        "https://claude.ai/public/artifacts/a-page-anyone-made",
        "https://www.anthropic.com/oauth/authorize",
        "https://claude.ai/oauth/authorize/../../public",
    ] {
        let mut driven = claude();
        let said = driven.read(&format!("If the browser didn't open, visit: {page}\n"));

        assert!(
            matches!(said.as_slice(), [Event::CouldNotFollow { said }] if said.contains(page)),
            "{page}: {said:?}"
        );
    }
}

/// The code pasted goes to the login as one line.
#[test]
fn the_pasted_code_goes_to_the_login_as_one_line() {
    let mut driven = asking();

    assert_eq!(
        driven.paste("  abc123#recorded \n"),
        Ok("abc123#recorded\n".to_owned())
    );
}

/// **Nothing but a code is passed on**: an empty paste, one with a space, a control character
/// or a line break inside it, and a paste before the login asked, are refused and nothing is
/// written.
#[test]
fn nothing_but_a_code_is_passed_on() {
    let mut driven = asking();
    for bad in [
        "",
        "   ",
        "abc def",
        "abc\ndef",
        "abc\u{1b}[2J",
        &"x".repeat(5000),
    ] {
        assert!(driven.paste(bad).is_err(), "{bad:?} was passed on");
    }
    assert!(claude().paste("abc#def").is_err());
}

/// **A code the harness refuses is said in its own words**, and another may be pasted.
#[test]
fn a_refused_code_is_said_in_the_harnesss_words_and_another_may_be_pasted() {
    let mut driven = asking();
    driven.paste("wrong").unwrap();

    assert_eq!(
        driven.read("Invalid code. Please make sure the full code was copied.\n"),
        [Event::Refused {
            said: "Invalid code. Please make sure the full code was copied.".to_owned()
        }]
    );
    assert!(driven.paste("abc#recorded").is_ok());
}

/// **"Signed in" is the harness's own status check's**: a login that ends well is checked, and
/// is signed in only once the check says so.
#[test]
fn signed_in_is_the_status_checks_answer() {
    let mut driven = asking();
    driven.paste("abc#recorded").unwrap();
    driven.read("Login successful.\n");

    assert_eq!(driven.ended(Some(0)), [Event::Checking]);
    assert_eq!(driven.checked(Standing::SignedIn), Event::SignedIn);

    let mut driven = asking();
    driven.paste("abc#recorded").unwrap();
    driven.ended(Some(0));
    assert!(matches!(
        driven.checked(Standing::SignedOut),
        Event::Failed { .. }
    ));
    let mut driven = asking();
    driven.ended(Some(0));
    assert!(matches!(
        driven.checked(Standing::CannotTell),
        Event::Failed { .. }
    ));
}

/// A login that fails says why, in the harness's words.
#[test]
fn a_failed_login_says_why_in_the_harnesss_words() {
    let mut driven = asking();
    driven.paste("abc#recorded").unwrap();
    driven.read("Login failed: the code has expired\n");

    assert_eq!(
        driven.ended(Some(1)),
        [Event::Failed {
            why: "the code has expired".to_owned()
        }]
    );
}

/// A sign-in that ends before its harness says anything says the last thing that was said: the
/// runner's purlis refusing it, say.
#[test]
fn a_sign_in_that_ends_early_says_the_last_thing_said() {
    let mut driven = claude();
    driven.read("purlis: the harness could not be fetched. Nothing was signed in.\n");

    assert_eq!(
        driven.ended(Some(1)),
        [Event::Failed {
            why:
                "the runner said: purlis: the harness could not be fetched. Nothing was signed in."
                    .to_owned()
        }]
    );
}

/// What the login prints on its error output is read in lines of its own: a warning printed in
/// the middle of the page's line does not cut the page.
#[test]
fn the_two_outputs_are_read_in_lines_of_their_own() {
    let mut driven = claude();
    let (start, rest) = recorded().split_at(60);
    let mut said = driven.read_from(Stream::Out, start);
    said.extend(driven.read_from(Stream::Err, "warning: slow network\n"));
    said.extend(driven.read_from(Stream::Out, rest));

    assert_eq!(
        said,
        [
            Event::Open {
                url: PAGE.to_owned()
            },
            Event::AskCode
        ]
    );
}

/// **What the runner says reads as it is**: no direction marks or invisible characters reach
/// the window, and what it says is said as the runner's.
#[test]
fn what_the_runner_says_carries_no_unseen_characters() {
    let mut driven = claude();
    driven.read("Login failed: \u{202e}evil\u{200b} text\u{1b}P1;2|payload\u{1b}\\\n");

    assert_eq!(
        driven.ended(Some(1)),
        [Event::Failed {
            why: "evil text".to_owned()
        }]
    );
}

/// **A login that prints without end is held to a bound**: a line is read once it is long
/// enough, and what is said back is cut.
#[test]
fn a_login_that_prints_without_end_is_held_to_a_bound() {
    let start = Instant::now();
    let mut driven = Driven::of("claude", start).unwrap();
    for _ in 0..1000 {
        assert!(driven.read(&"x".repeat(4096)).is_empty());
    }

    let Some(Event::CouldNotFollow { said }) = driven.waited(start + PAGE_WITHIN) else {
        panic!("it could be followed");
    };
    assert!(said.chars().count() <= 4096, "{}", said.len());
}

/// **What purlis does not recognise within its wait is never guessed at**: it cannot follow
/// the sign-in, and says what the harness printed.
#[test]
fn what_is_not_recognised_in_time_cannot_be_followed() {
    let start = Instant::now();
    let mut driven = Driven::of("claude", start).unwrap();
    driven.read("Choose your sign-in method: [1] Claude [2] Console\n");

    assert_eq!(driven.waited(start + Duration::from_secs(5)), None);
    assert!(matches!(
        driven.waited(start + PAGE_WITHIN),
        Some(Event::CouldNotFollow { said }) if said.contains("Choose your sign-in method")
    ));
    assert_eq!(driven.waited(start + PAGE_WITHIN * 2), None, "said once");
}

/// Once the page is open, the person has their time: the wait for the page no longer applies.
#[test]
fn once_the_page_is_open_the_person_has_their_time() {
    let start = Instant::now();
    let mut driven = Driven::of("claude", start).unwrap();
    driven.read(recorded());

    assert_eq!(driven.waited(start + PAGE_WITHIN * 2), None);
}

/// Only logins purlis follows are driven: Claude Code's and opencode's Copilot one, yet.
#[test]
fn only_the_logins_purlis_follows_are_driven() {
    for kind in ["codex", "opencode", "gemini"] {
        assert!(Driven::of(kind, Instant::now()).is_none(), "{kind}");
    }
    assert!(Driven::of_plan("opencode", Some("openai"), Instant::now()).is_none());
    assert!(Driven::of_plan("opencode", Some("github-copilot"), Instant::now()).is_some());
    assert!(Driven::of_plan("claude", Some("github-copilot"), Instant::now()).is_none());
}

/// What opencode 1.18.33's `auth login --provider github-copilot` printed with its input a pipe,
/// from its first line to its spinner: its question, answered, then the device code. The code
/// here is GitHub's documentation's.
const COPILOT: &str = include_str!("opencode-1.18.33-copilot-login.txt");

/// The line opencode's spinner ends on once GitHub accepted the code, after its redraws.
const COPILOT_SIGNED_IN: &str = "\u{1b}[999D\u{1b}[J\u{25c7}  Login successful\n";

const DEVICE_PAGE: &str = "https://github.com/login/device";

fn copilot() -> Driven {
    Driven::of_plan("opencode", Some("github-copilot"), Instant::now())
        .expect("opencode's Copilot sign-in is driven")
}

fn device_code() -> Event {
    Event::DeviceCode {
        url: DEVICE_PAGE.to_owned(),
        code: "WDJB-MJHT".to_owned(),
    }
}

/// **opencode's Copilot login says its code and its page together**, once, read in any pieces:
/// the one thing it asks first is answered by purlis, for GitHub.com.
#[test]
fn copilots_login_says_its_device_code_and_page_once() {
    let mut whole = copilot();
    assert_eq!(whole.read(COPILOT), [device_code()]);
    assert_eq!(whole.reply(), Some("\r"));
    assert_eq!(whole.reply(), None);

    let mut cut = copilot();
    let mut said = Vec::new();
    let mut replies = 0;
    let mut held = Vec::new();
    for piece in COPILOT.as_bytes().chunks(7) {
        held.extend_from_slice(piece);
        // Cut between bytes, as a read may be: whole characters are fed, as `reading` feeds them.
        if let Ok(text) = std::str::from_utf8(&held) {
            said.extend(cut.read(text));
            held.clear();
        }
        replies += usize::from(cut.reply().is_some());
    }
    assert_eq!(said, [device_code()]);
    assert_eq!(replies, 1);
}

/// **Its one question is answered only as it shows GitHub.com chosen**: any other question, or
/// another choice, is left for [`Event::CouldNotFollow`] to say.
#[test]
fn a_question_purlis_does_not_know_is_left_unanswered() {
    for asked in [
        "\u{25c6}  Select GitHub deployment type\n\u{2502}  \u{25cf} GitHub Enterprise\n",
        "\u{25c6}  Some question of a later version\n\u{2502}  \u{25cf} Yes\n",
    ] {
        let mut driven = copilot();
        assert_eq!(driven.read(asked), []);
        assert_eq!(driven.reply(), None, "{asked}");
    }
}

/// **A code or a page that is not plainly GitHub's is never shown**: the login cannot be followed.
#[test]
fn a_device_code_is_shown_only_for_githubs_own_page() {
    for (page, code) in [
        ("https://github.com.evil.example/login/device", "WDJB-MJHT"),
        ("http://github.com/login/device", "WDJB-MJHT"),
        ("https://github.com/login/device/other", "WDJB-MJHT"),
        ("https://github.com/login/device", "not a code"),
        ("https://github.com/login/device", "WDJB<b>MJHT"),
    ] {
        let mut driven = copilot();
        let said = driven.read(&format!(
            "\u{25cf}  Go to: {page}\n\u{2502}\n\u{25cf}  Enter code: {code}\n"
        ));
        assert!(
            matches!(said.as_slice(), [Event::CouldNotFollow { .. }]),
            "{page} {code}: {said:?}"
        );
        assert!(driven.is_over());
    }
}

/// **A code with no page is not followed**: nothing is shown, and the wait runs out.
#[test]
fn a_device_code_with_no_page_shows_nothing() {
    let mut driven = copilot();
    assert_eq!(driven.read("\u{25cf}  Enter code: WDJB-MJHT\n"), []);
    assert!(!driven.is_over());
}

/// **Once GitHub accepts the code**, its status check says whether it is signed in.
#[test]
fn copilots_login_is_checked_once_it_says_it_signed_in() {
    let mut driven = copilot();
    driven.read(COPILOT);
    assert_eq!(driven.read(COPILOT_SIGNED_IN), []);

    assert_eq!(driven.ended(Some(0)), [Event::Checking]);
    assert_eq!(driven.checked(Standing::SignedIn), Event::SignedIn);
}

/// **Exit status 0 is not enough for a login that also exits 0 having failed**: opencode does, and
/// its status check is whether a file is there, which a sign-in to its other plan leaves. Without
/// its own words for success, the check is never asked.
#[test]
fn a_copilot_login_that_failed_with_status_0_is_not_signed_in() {
    let mut driven = copilot();
    driven.read(COPILOT);
    driven.read("\u{1b}[999D\u{1b}[J\u{25a0}  Failed to authorize\n");

    assert_eq!(
        driven.ended(Some(0)),
        [Event::Failed {
            why: "Failed to authorize".to_owned()
        }]
    );
    assert!(driven.is_over());
}

/// A login that says neither is not taken for signed in either.
#[test]
fn a_copilot_login_that_says_nothing_of_how_it_ended_is_not_signed_in() {
    let mut driven = copilot();
    driven.read(COPILOT);

    let said = driven.ended(Some(0));

    assert!(
        matches!(said.as_slice(), [Event::Failed { .. }]),
        "{said:?}"
    );
}

/// opencode's own error line is read as its reason.
#[test]
fn a_copilot_login_that_could_not_reach_github_says_so() {
    let mut driven = copilot();
    driven.read_from(
        Stream::Err,
        "\u{1b}[91m\u{1b}[1mError: \u{1b}[0mFailed to authorize: Unable to connect.\n",
    );

    assert_eq!(
        driven.ended(Some(1)),
        [Event::Failed {
            why: "Failed to authorize: Unable to connect.".to_owned()
        }]
    );
}

/// A spinner redraws on one line with no line break: what is kept of it is its last frame, so what
/// the login said last reads as words.
#[test]
fn a_spinners_redraws_are_read_as_its_last_frame() {
    let mut driven = copilot();
    driven.read(COPILOT);
    driven.read("\u{1b}[999D\u{1b}[J\u{25a0}  Something opencode says\n");

    let said = driven.ended(Some(1));

    assert_eq!(
        said,
        [Event::Failed {
            why: "the runner said: \u{25a0}  Something opencode says".to_owned()
        }]
    );
}

/// Claude Code's login is told by its status alone, as before.
#[test]
fn a_login_with_no_words_for_success_goes_by_its_status() {
    let mut driven = asking();
    assert_eq!(driven.ended(Some(0)), [Event::Checking]);
}

/// **A login driven end to end**: a stand-in prints the recorded output, takes the code, says
/// it signed in, and the status check agrees.
#[cfg(unix)]
#[test]
fn a_login_is_driven_end_to_end() {
    let login = stand_in(
        "printf '%s' \"$PRINTED\"; read code; [ \"$code\" = 'abc#recorded' ] || exit 3; \
         echo 'Login successful.'",
    );
    let driving = drive(&login, "claude", || Standing::SignedIn).unwrap();

    assert_eq!(
        next(&driving),
        Event::Open {
            url: PAGE.to_owned()
        }
    );
    assert_eq!(next(&driving), Event::AskCode);
    driving.paste("abc#recorded").unwrap();
    assert_eq!(next(&driving), Event::Checking);
    assert_eq!(next(&driving), Event::SignedIn);
}

/// **opencode's Copilot login driven end to end**: the stand-in reads the answer to its question
/// before it goes on, and the code is shown while it waits for GitHub.
#[cfg(unix)]
#[test]
fn a_copilot_login_is_driven_end_to_end() {
    let mut login = stand_in(
        "printf '\\342\\227\\206  Select GitHub deployment type\\n\\342\\224\\202  \\342\\227\\217 \
         GitHub.com (Public)\\n'; answer=$(head -c 1); [ \"$answer\" = \"$(printf '\\r')\" ] || exit 3; \
         echo 'Go to: https://github.com/login/device'; echo 'Enter code: WDJB-MJHT'; \
         echo 'Login successful'",
    );
    login.env.clear();
    let driving = drive_with(&login, "opencode", Some("github-copilot"), || {
        Standing::SignedIn
    })
    .unwrap();

    assert_eq!(next(&driving), device_code());
    assert_eq!(next(&driving), Event::Checking);
    assert_eq!(next(&driving), Event::SignedIn);
}

/// A login that ended is said to have ended, though something it started holds its output open.
#[cfg(unix)]
#[test]
fn a_login_that_ended_is_not_waited_on_past_what_it_left_running() {
    let login = stand_in("printf '%s' \"$PRINTED\"; sleep 20 & exit 0");
    let driving = drive(&login, "claude", || Standing::SignedIn).unwrap();

    assert!(matches!(next(&driving), Event::Open { .. }));
    assert_eq!(next(&driving), Event::AskCode);
    assert_eq!(next(&driving), Event::Checking);
    assert_eq!(next(&driving), Event::SignedIn);
}

/// A sign-in cancelled ends its login, and says it was cancelled.
#[cfg(unix)]
#[test]
fn a_cancelled_sign_in_ends_its_login() {
    let login = stand_in("printf '%s' \"$PRINTED\"; exec sleep 600");
    let driving = drive(&login, "claude", || Standing::SignedIn).unwrap();
    assert!(matches!(next(&driving), Event::Open { .. }));
    assert_eq!(next(&driving), Event::AskCode);

    driving.cancel();

    assert_eq!(next(&driving), Event::Cancelled);
}

#[cfg(unix)]
fn stand_in(script: &str) -> Login {
    Login {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), script.to_owned()],
        env: vec![("PRINTED".to_owned(), recorded().to_owned())],
        cleared: Vec::new(),
    }
}

#[cfg(unix)]
fn next(driving: &Driving) -> Event {
    let until = Instant::now() + Duration::from_secs(30);
    while Instant::now() < until {
        if let Some(event) = driving.next(Duration::from_secs(1)) {
            return event;
        }
    }
    panic!("the sign-in said nothing in 30s");
}
