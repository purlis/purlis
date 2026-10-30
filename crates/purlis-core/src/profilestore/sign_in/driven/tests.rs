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

/// Only logins purlis follows are driven: opencode's to a plan of its own, Copilot's not yet.
#[test]
fn only_the_logins_purlis_follows_are_driven() {
    assert!(Driven::of("codex", Instant::now()).is_some());
    assert!(Driven::of_plan("opencode", Some("openai"), Instant::now()).is_some());
    for (kind, plan) in [
        ("opencode", None),
        ("opencode", Some("github-copilot")),
        ("gemini", None),
    ] {
        assert!(
            Driven::of_plan(kind, plan, Instant::now()).is_none(),
            "{kind} {plan:?}"
        );
    }
}

/// What Codex 0.163.0's `codex login` prints with its input a pipe, as its own code writes it.
const CODEX: &str = include_str!("codex-0.163.0-login.txt");
/// What opencode 1.18.33's ChatGPT `auth login` prints, as its own code writes it.
const OPENCODE: &str = include_str!("opencode-1.18.33-chatgpt-login.txt");

const STATE: &str = "Zm9yLXRoaXMtc2lnbi1pbi1hbG9uZQ";

fn codex() -> Driven {
    Driven::of("codex", Instant::now()).expect("Codex's sign-in is driven")
}

fn opencode() -> Driven {
    Driven::of_plan("opencode", Some("openai"), Instant::now())
        .expect("opencode's ChatGPT sign-in is driven")
}

/// The page line of a recorded login.
fn page_of(printed: &str) -> String {
    let at = printed.find("https://").unwrap();
    printed[at..].split_whitespace().next().unwrap().to_owned()
}

/// **Codex's login is approved in the browser**: its page is opened, it waits on its port for
/// the redirect of this login's state, and nothing is asked to be pasted.
#[test]
fn codexs_login_is_approved_in_the_browser() {
    assert_eq!(
        codex().read(CODEX),
        [Event::Approve {
            url: page_of(CODEX),
            port: 1455,
            state: STATE.to_owned()
        }]
    );
}

/// **opencode's ChatGPT login is approved the same way**.
#[test]
fn opencodes_chatgpt_login_is_approved_in_the_browser() {
    assert_eq!(
        opencode().read(OPENCODE),
        [Event::Approve {
            url: page_of(OPENCODE),
            port: 1455,
            state: STATE.to_owned()
        }]
    );
}

/// Read in pieces, on either output, it says the same, once.
#[test]
fn a_login_approved_in_the_browser_is_read_in_pieces() {
    let mut driven = codex();
    let mut said = Vec::new();
    for piece in CODEX.as_bytes().chunks(11) {
        said.extend(driven.read_from(Stream::Err, &String::from_utf8_lossy(piece)));
    }

    assert!(
        matches!(said.as_slice(), [Event::Approve { .. }]),
        "{said:?}"
    );
}

/// **A page that does not send the browser back as purlis can carry is never opened**: to
/// another address or port, with no state, two, or one no login made.
#[test]
fn a_page_that_sends_the_browser_elsewhere_cannot_be_followed() {
    let page = page_of(CODEX);
    let elsewhere = "redirect_uri=https%3A%2F%2Fevil.example%2Fauth%2Fcallback";
    let own = "redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback";
    for bad in [
        page.replace(own, elsewhere),
        page.replace("%3A1455", "%3A1456"),
        page.replace("localhost", "127.0.0.1"),
        page.replace(own, &format!("{own}&{elsewhere}")),
        page.replace(&format!("&state={STATE}"), ""),
        page.replace(&format!("state={STATE}"), &format!("state=a&state={STATE}")),
        page.replace(STATE, "short"),
        page.replace(STATE, "not+written%20plainly-at-all"),
        page.replace("response_type=code", "response_type=token"),
        page.replace("auth.openai.com", "auth.openai.com.evil.example"),
        page.replacen("/oauth/authorize", "/elsewhere", 1),
    ] {
        let said = codex().read(&format!("{bad}\n"));

        assert!(
            matches!(said.as_slice(), [Event::CouldNotFollow { .. }]),
            "{bad}: {said:?}"
        );
    }
}

/// Nothing is pasted to a login that waits for the browser.
#[test]
fn nothing_is_pasted_to_a_login_that_waits_for_the_browser() {
    let mut driven = codex();
    driven.read(CODEX);

    assert!(driven.paste("a-code").is_err());
}

/// The person has their time at the page, and a login that ended well is checked.
#[test]
fn a_login_approved_in_the_browser_ends_with_its_status_check() {
    let start = Instant::now();
    let mut driven = Driven::of("codex", start).unwrap();
    driven.read(CODEX);
    assert_eq!(driven.waited(start + PAGE_WITHIN * 2), None);

    assert_eq!(driven.carried(Ok(())), None);
    driven.read("Successfully logged in\n");
    assert_eq!(driven.ended(Some(0)), [Event::Checking]);
    assert_eq!(driven.checked(Standing::SignedIn), Event::SignedIn);
}

/// **A redirect that was not carried fails the sign-in**, and says why.
#[test]
fn a_redirect_not_carried_fails_the_sign_in() {
    let mut driven = codex();
    driven.read(CODEX);

    assert_eq!(
        driven.carried(Err("no sign-in is waiting on the runner".to_owned())),
        Some(Event::Failed {
            why: "no sign-in is waiting on the runner".to_owned()
        })
    );
    assert!(driven.is_over());
}

/// A login whose own port is taken on the runner fails in its words.
#[test]
fn a_login_that_fails_says_so_in_codexs_and_opencodes_words() {
    let mut driven = codex();
    driven.read("Error logging in: Port 127.0.0.1:1455 is already in use\n");
    assert_eq!(
        driven.ended(Some(1)),
        [Event::Failed {
            why: "Port 127.0.0.1:1455 is already in use".to_owned()
        }]
    );

    let mut driven = opencode();
    driven.read("\u{1b}[91m\u{1b}[1mError: \u{1b}[0mFailed to authorize: Failed to start server\n");
    assert_eq!(
        driven.ended(Some(1)),
        [Event::Failed {
            why: "Failed to authorize: Failed to start server".to_owned()
        }]
    );
}

/// A taken port on this device ends it, and is its last step.
#[test]
fn a_taken_port_is_the_last_step() {
    let mut driven = codex();
    driven.read(CODEX);

    let said = driven.port_taken(1455);

    assert_eq!(said, Event::PortTaken { port: 1455 });
    assert!(said.is_last());
    assert!(driven.is_over());
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

/// A stand-in that prints a login's page and waits, as Codex's does, until `gate` exists.
#[cfg(unix)]
fn waiting_login(gate: &std::path::Path) -> Login {
    Login {
        program: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            "printf '%s' \"$PRINTED\"; while [ ! -e \"$GATE\" ]; do sleep 0.05; done; \
             echo 'Successfully logged in'"
                .to_owned(),
        ],
        env: vec![
            ("PRINTED".to_owned(), CODEX.to_owned()),
            ("GATE".to_owned(), gate.display().to_string()),
        ],
        cleared: Vec::new(),
    }
}

/// Whether anything listens on this device's loopback at `port`.
#[cfg(unix)]
fn listens(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// A port nothing listens on now.
#[cfg(unix)]
fn free_port() -> u16 {
    let free = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    free.local_addr().unwrap().port()
}

/// Drives a stand-in for Codex's login, listening on `port` as it would on its own.
#[cfg(unix)]
fn driving_on(port: u16, login: &Login, carry: Carry) -> Driving {
    let forwarding = Arc::new(Forwarding {
        carry,
        listen_on: Some(port),
        forward: Mutex::new(Forwarded::Not),
    });
    run(login, codex(), Some(forwarding), || Standing::SignedIn).unwrap()
}

/// **A login approved in the browser, end to end**: the page is told once this device listens,
/// the browser's redirect is carried, the port closes, and the login's end is checked.
#[cfg(unix)]
#[test]
fn a_login_is_approved_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let gate = dir.path().join("carried");
    let port = free_port();
    let carry: Carry = {
        let gate = gate.clone();
        Arc::new(move |target: &str| {
            assert!(target.starts_with("/auth/callback?code=c&state="));
            std::fs::write(&gate, "").map_err(|e| e.to_string())
        })
    };
    let driving = driving_on(port, &waiting_login(&gate), carry);

    assert!(matches!(next(&driving), Event::Approve { .. }));
    let mut browser = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        browser,
        "GET /auth/callback?code=c&state={STATE} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"
    )
    .unwrap();

    assert_eq!(next(&driving), Event::Checking);
    assert_eq!(next(&driving), Event::SignedIn);
    assert!(!listens(port), "it is closed after");
}

/// **A sign-in that ends closes the port**: cancelled, nothing listens, and nothing was carried.
#[cfg(unix)]
#[test]
fn a_cancelled_sign_in_closes_the_port() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let carry: Carry = Arc::new(|_: &str| panic!("nothing is carried"));
    let driving = driving_on(port, &waiting_login(&dir.path().join("never")), carry);
    assert!(matches!(next(&driving), Event::Approve { .. }));
    assert!(listens(port), "it listens from before the page opens");

    driving.cancel();

    assert_eq!(next(&driving), Event::Cancelled);
    assert!(!listens(port));
}

/// **A taken port is said, the page is not opened, and the login is ended**.
#[cfg(unix)]
#[test]
fn a_taken_port_is_said_and_no_page_opens() {
    let dir = tempfile::tempdir().unwrap();
    let held = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = held.local_addr().unwrap().port();
    let driving = driving_on(
        port,
        &waiting_login(&dir.path().join("never")),
        Arc::new(|_: &str| Ok(())),
    );

    assert_eq!(next(&driving), Event::PortTaken { port: 1455 });
    drop(held);
}

/// A login that waits for the browser is not driven with no way to carry its answer.
#[test]
fn a_login_that_waits_for_the_browser_needs_a_way_to_carry_its_answer() {
    let login = Login {
        program: "/bin/true".to_owned(),
        args: Vec::new(),
        env: Vec::new(),
        cleared: Vec::new(),
    };

    assert!(drive(&login, "codex", || Standing::SignedIn).is_err());
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
