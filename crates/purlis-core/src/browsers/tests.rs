// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::{EngineBrowsers, KEEPER, browsers_in, engines_in};

#[test]
fn engines_and_browsers_live_apart_under_the_data_home() {
    let data = Path::new("/home/me/.local/share/purlis");
    assert_eq!(engines_in(data), data.join("engines/chrome-for-testing"));
    assert_eq!(browsers_in(data), data.join("browsers"));
}

#[test]
fn the_keeper_is_this_binary_started_in_its_keepers_part() {
    let keeper = EngineBrowsers::keeper("/bin/purlis".into());
    assert_eq!(keeper.program, Path::new("/bin/purlis"));
    assert_eq!(keeper.args, [std::ffi::OsString::from(KEEPER)]);
}

#[test]
fn each_browsers_data_folder_is_made_under_the_data_home_it_was_given() {
    use purlis_host::Browsers;
    let browsers = EngineBrowsers::new("/home/me/.local/share/purlis".into(), "/bin/purlis".into());
    assert_eq!(
        browsers.data_folders().unwrap(),
        Path::new("/home/me/.local/share/purlis/browsers")
    );
}

// ---- An engine's age at a browser's start (#5413) ----

mod an_engines_age_at_a_start {
    use std::io;
    use std::sync::Mutex;

    use chrono::{DateTime, TimeDelta};
    use purlis_browser_engine::install::{
        EngineSettings, Engines, Fetch, Manifest, NoSigner, Platform, Refused,
    };

    use super::super::{engines_clock, prepared};

    /// Answers nothing, as a read that met a 504 or no network, and keeps what it was asked.
    #[derive(Default)]
    struct Unreachable(Mutex<Vec<String>>);

    impl Unreachable {
        fn asked(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    impl Fetch for Unreachable {
        fn fetch(&self, url: &str) -> io::Result<Box<dyn io::Read + '_>> {
            self.0.lock().unwrap().push(url.to_owned());
            Err(io::Error::other(format!("{url} answered 504")))
        }
    }

    /// What a start answered, and what each of its reads was asked.
    struct Started {
        answer: Result<(), Refused>,
        manifests: Vec<String>,
        archives: Vec<String>,
    }

    /// The start this build's pin gets `after` it was issued, neither read answering, on a
    /// machine that has installed nothing.
    fn started(after: TimeDelta) -> Started {
        let home = tempfile::tempdir().unwrap();
        let shipped = Manifest::shipped();
        let now = shipped.issued + after;
        let engines = Engines::new(home.path(), Platform::Linux64, shipped, Box::new(NoSigner));
        let (manifests, archives) = (Unreachable::default(), Unreachable::default());
        let answer = prepared(
            &engines,
            &EngineSettings::default(),
            &manifests,
            &archives,
            now,
        );
        Started {
            answer: answer.map(|_| ()),
            manifests: manifests.asked(),
            archives: archives.asked(),
        }
    }

    #[test]
    fn past_the_limit_with_no_manifest_to_read_the_shipped_engine_is_refused_for_its_age() {
        let Started {
            answer,
            manifests,
            archives,
        } = started(TimeDelta::days(22));
        let refused = answer.expect_err("an engine past the limit does not start");
        assert!(
            matches!(refused, Refused::Outdated { days: 22 }),
            "{refused:?}"
        );
        assert_eq!(
            refused.to_string(),
            "The browser engine is 22 days behind, so it was not started. Update now, or allow \
             an outdated engine in Settings."
        );
        assert_eq!(
            manifests.len(),
            1,
            "the published manifest was tried first: {manifests:?}"
        );
        assert!(archives.is_empty(), "nothing downloaded: {archives:?}");
    }

    #[test]
    fn on_the_day_its_pin_was_issued_a_start_reads_no_manifest() {
        let Started {
            answer,
            manifests,
            archives,
        } = started(TimeDelta::zero());
        assert!(
            matches!(answer, Err(Refused::Download(_))),
            "it went on to download the pinned engine: {answer:?}"
        );
        assert!(manifests.is_empty(), "no manifest read: {manifests:?}");
        assert_eq!(archives.len(), 1, "{archives:?}");
    }

    #[test]
    fn only_a_debug_build_takes_its_engines_clock_from_the_variable() {
        let real = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
            .unwrap()
            .to_utc();
        let asked = "2026-10-14T11:19:22Z";
        let named = DateTime::parse_from_rfc3339(asked).unwrap().to_utc();
        assert_eq!(engines_clock(true, Some(asked), real), named);
        assert_eq!(
            engines_clock(false, Some(asked), real),
            real,
            "a release build refuses by the real time, whatever its environment says"
        );
        assert_eq!(engines_clock(true, None, real), real);
        assert_eq!(engines_clock(true, Some("soon"), real), real);
    }
}

// ---- A chat's acts, its answers and its reach (#2013) ----

use std::time::Duration;

use purlis_browser_engine::drive::{
    self, Answer, Call, Content, DragTo, Field, PageId, PageInfo, Ref, Settled,
};
use purlis_host::protocol::browsing::{self as wire, Act, Browsed, BrowserReach, PageSaid};
use purlis_host::protocol::session::{Layer, Listed};

use super::{aim_of, browsed, call_of, input_of, reach_of};
use crate::sandbox::browser_reach::{LoginSites, Site};

#[test]
fn the_acts_a_chat_asks_become_the_drivers_own_calls() {
    assert_eq!(
        call_of(Act::Click {
            target: "e3".into()
        }),
        Ok(Call::Click {
            target: Ref::new("e3")
        })
    );
    assert_eq!(
        call_of(Act::Snapshot {
            page: Some("p2".into()),
            within: None,
            from: None,
            lines: Some(50),
        }),
        Ok(Call::Snapshot {
            page: Some(PageId::new("p2")),
            within: None,
            from: 1,
            lines: Some(50),
        }),
        "a snapshot with no start reads from the first line"
    );
    assert_eq!(
        call_of(Act::WaitForText {
            page: None,
            text: "Saved".into(),
            gone: true,
            within_ms: 2500,
        }),
        Ok(Call::WaitForText {
            page: None,
            text: "Saved".into(),
            gone: true,
            within: Duration::from_millis(2500),
        })
    );
    assert_eq!(
        call_of(Act::Drag {
            from: "e1".into(),
            to: wire::DragTo::Offset { x: 40, y: -8 },
        }),
        Ok(Call::Drag {
            from: Ref::new("e1"),
            to: DragTo::Offset { x: 40.0, y: -8.0 },
        })
    );
    assert_eq!(
        call_of(Act::Type {
            page: None,
            fields: vec![wire::Typed {
                into: None,
                text: "hi".into(),
                replace: true,
            }],
        }),
        Ok(Call::Type {
            page: None,
            fields: vec![Field {
                into: None,
                text: "hi".into(),
                replace: true,
            }],
        })
    );
}

#[test]
fn an_address_that_is_not_a_web_address_is_refused_in_words_before_the_driver_hears_it() {
    let refused = call_of(Act::Open {
        address: "file:///etc/passwd".into(),
    })
    .unwrap_err();

    assert!(refused.contains("http and https"), "{refused}");
}

#[test]
fn the_drivers_answer_is_said_back_whole() {
    let page = PageInfo {
        id: PageId::new("p1"),
        address: "https://docs.example.com/".into(),
        title: "Docs".into(),
        current: true,
    };
    let answer = Answer {
        page: Some(page.clone()),
        settled: Settled::No(vec!["a request still open".into()]),
        opened: vec![page],
        content: Content::Text("- heading \"Docs\" [e1]".into()),
        note: Some("the page did not load".into()),
        repeated: true,
        asked: vec!["news.example.org:443".into()],
        downloads: Vec::new(),
    };

    let said = PageSaid {
        page: "p1".into(),
        address: "https://docs.example.com/".into(),
        title: "Docs".into(),
        current: true,
    };
    assert_eq!(
        browsed(answer, usize::MAX),
        Browsed {
            page: Some(said.clone()),
            settled: false,
            waiting_for: vec!["a request still open".into()],
            opened: vec![said],
            text: Some("- heading \"Docs\" [e1]".into()),
            note: Some("the page did not load".into()),
            repeated: true,
            asked: vec!["news.example.org:443".into()],
            ..Browsed::default()
        }
    );
}

#[test]
fn a_browser_reaches_what_its_chat_lists_and_a_new_site_only_on_the_open_web() {
    let listed = BrowserReach {
        listed: vec![Listed {
            host: "docs.example.com".into(),
            layer: Layer::Open,
        }],
        never: Vec::new(),
        withdrawn: Vec::new(),
        open_web: false,
        answered: Vec::new(),
    };

    let (reach, new_sites) = reach_of(Some(&listed)).unwrap();
    assert_eq!(reach.hosts(), ["docs.example.com"]);
    assert!(!new_sites.carries(&Site::of("news.example.org", 443)));

    let (_, new_sites) = reach_of(Some(&BrowserReach {
        open_web: true,
        ..listed
    }))
    .unwrap();
    assert!(new_sites.carries(&Site::of("news.example.org", 443)));

    let (reach, new_sites) = reach_of(None).unwrap();
    assert!(reach.hosts().is_empty(), "none given, nothing listed");
    assert!(!new_sites.carries(&Site::of("news.example.org", 443)));
}

#[test]
fn a_browser_reaches_no_port_of_this_machine_its_chat_had_withdrawn() {
    let given = BrowserReach {
        listed: vec![
            Listed {
                host: "127.0.0.1:5173".into(),
                layer: Layer::You,
            },
            Listed {
                host: "127.0.0.1:8080".into(),
                layer: Layer::You,
            },
        ],
        withdrawn: vec![5173],
        ..BrowserReach::default()
    };

    let (reach, _) = reach_of(Some(&given)).unwrap();

    assert!(!reach.decide("127.0.0.1", 5173, &[], &[]).carries());
    assert!(reach.decide("127.0.0.1", 8080, &[], &[]).carries());
}

#[test]
fn the_persons_key_becomes_the_drivers_own_event_aimed_where_they_aimed_it() {
    let input = wire::Input::Key {
        aim: wire::Aim {
            page: "p2".into(),
            document: 9,
        },
        action: wire::KeyAction::Down,
        key: "A".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 8,
    };

    assert_eq!(
        input_of(&input),
        drive::PersonInput::Key {
            aim: drive::Aim {
                page: drive::PageId::new("p2"),
                document: 9,
            },
            down: true,
            key: "A".into(),
            code: "KeyA".into(),
            key_code: 65,
            modifiers: 8,
        }
    );
}

#[test]
fn the_persons_wheel_becomes_the_drivers_own_wheel_by_the_same_distance() {
    let input = wire::Input::Mouse {
        aim: wire::Aim {
            page: "p1".into(),
            document: 3,
        },
        action: wire::MouseAction::Wheel,
        x: 640,
        y: 400,
        button: wire::Button::None,
        clicks: 0,
        modifiers: 0,
        delta_x: 0,
        delta_y: -120,
    };

    assert_eq!(
        input_of(&input),
        drive::PersonInput::Mouse {
            aim: drive::Aim {
                page: drive::PageId::new("p1"),
                document: 3,
            },
            kind: drive::MouseKind::Wheel,
            x: 640.0,
            y: 400.0,
            button: drive::MouseButton::None,
            clicks: 0,
            modifiers: 0,
            delta_x: 0.0,
            delta_y: -120.0,
        }
    );
}

#[test]
fn a_frame_names_its_page_and_document_in_the_protocols_words() {
    let aim = drive::Aim {
        page: drive::PageId::new("p4"),
        document: 12,
    };
    assert_eq!(
        aim_of(&aim),
        wire::Aim {
            page: "p4".into(),
            document: 12,
        }
    );
}

// ---- The engine kept current in the background (#2073) ----

use std::cell::RefCell;

use purlis_browser_engine::install::{
    EngineSettings, Engines, Fetch, Manifest, NoSigner, PUBLISHED_BUNDLE, PUBLISHED_MANIFEST,
    Platform, Refused,
};

use super::keep_current;
use crate::netlog::{Feature, Party};

/// Serves only the published manifest, and remembers what it was asked for.
#[derive(Default)]
struct Asked(RefCell<Vec<String>>);

impl Fetch for Asked {
    fn fetch(&self, url: &str) -> std::io::Result<Box<dyn std::io::Read + '_>> {
        self.0.borrow_mut().push(url.to_owned());
        if url == PUBLISHED_MANIFEST {
            Ok(Box::new(std::io::Cursor::new(b"{}".to_vec())))
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "404"))
        }
    }
}

fn engines(root: &Path) -> Engines {
    Engines::new(
        root,
        Platform::Linux64,
        Manifest::shipped(),
        Box::new(NoSigner),
    )
}

#[test]
fn a_machine_that_never_used_the_pinned_engine_is_never_checked_for_one() {
    let root = tempfile::tempdir().unwrap();
    let asked = Asked::default();
    let checked = keep_current(
        &engines(&root.path().join("engines")),
        &EngineSettings::default(),
        &asked,
        &asked,
        chrono::Utc::now(),
    );
    assert!(checked.is_none());
    assert!(asked.0.borrow().is_empty(), "nothing reached the network");
}

#[test]
fn a_machine_with_the_pinned_engine_installed_reads_the_published_manifest_and_its_bundle() {
    let root = tempfile::tempdir().unwrap();
    let installed = root.path().join("155.0.8059.79-linux64");
    std::fs::create_dir_all(&installed).unwrap();
    std::fs::write(installed.join("installed.json"), r#"{"sha256":"ab"}"#).unwrap();
    let (manifests, archives) = (Asked::default(), Asked::default());
    let checked = keep_current(
        &engines(root.path()),
        &EngineSettings::default(),
        &manifests,
        &archives,
        chrono::Utc::now(),
    );
    assert!(
        matches!(checked, Some(Err(Refused::Download(_)))),
        "{checked:?}"
    );
    assert_eq!(
        *manifests.0.borrow(),
        [PUBLISHED_MANIFEST.to_owned(), PUBLISHED_BUNDLE.to_owned()]
    );
    assert!(archives.0.borrow().is_empty());
}

#[test]
fn the_engines_archive_is_a_third_partys_and_the_manifest_is_purliss_own() {
    assert_eq!(Feature::BrowserEngine.party(), Party::ThirdParty);
    assert_eq!(
        serde_json::to_value(Feature::BrowserEngine).unwrap(),
        "browser-engine"
    );
    let path = PUBLISHED_MANIFEST
        .strip_prefix("https://github.com")
        .unwrap();
    assert!(crate::netlog::is_charters("github.com", path));
}

// ---- A login purlis performs (#2016) ----

#[test]
fn a_login_is_filled_on_its_addresss_origin_and_bound_to_its_site() {
    use purlis_host::protocol::browsing::{LogIn, Secret};
    let (login, site) = super::login_of(&LogIn {
        address: "https://shop.example.test/login?next=/cart".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: false,
        form: None,
        start: None,
    })
    .unwrap();
    assert_eq!(login.origin, "https://shop.example.test");
    assert_eq!(site, LoginSites::page(Site::of("shop.example.test", 443)));
    assert!(login.user.is_none());
    assert_eq!(login.back_to, None);
    let local = super::login_of(&LogIn {
        address: "http://127.0.0.1:8123/login".into(),
        user: Some(Secret::new("fixture-user".into())),
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: false,
        form: None,
        start: None,
    })
    .unwrap();
    assert_eq!(local.0.origin, "http://127.0.0.1:8123");
    assert_eq!(local.1, LoginSites::page(Site::of("127.0.0.1", 8123)));
    let refused = super::login_of(&LogIn {
        address: "file:///etc/passwd".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: false,
        form: None,
        start: None,
    })
    .unwrap_err();
    assert!(refused.contains("not one a browser opens"), "{refused}");
    // Neither the driver's login nor what the window handed says its values.
    assert!(!format!("{login:?}").contains("fixture-word"));
}

#[test]
fn a_login_is_refused_on_plain_http_but_this_machines_loopback() {
    use purlis_host::protocol::browsing::{LogIn, Secret};
    let refused = super::login_of(&LogIn {
        address: "http://shop.example.test/login".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: false,
        form: None,
        start: None,
    })
    .unwrap_err();
    assert!(refused.contains("plain http"), "{refused}");
    let (login, site) = super::login_of(&LogIn {
        address: "https://bücher.example/login".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: false,
        form: None,
        start: None,
    })
    .unwrap();
    assert_eq!(login.origin, "https://xn--bcher-kva.example");
    assert_eq!(
        site,
        LoginSites::page(Site::of("xn--bcher-kva.example", 443))
    );
}

/// #4995: a login whose form is on another host fills on the form's origin alone, is held to its
/// page's site and, while it signs in, its form's, and has gone through once the browser is back
/// on its page.
#[test]
fn a_login_whose_form_is_elsewhere_fills_there_and_is_held_to_both_sites() {
    use purlis_host::protocol::browsing::{LogIn, Secret};
    let login = |form: &str| LogIn {
        address: "https://app.example.test/".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: true,
        form: Some(form.into()),
        start: None,
    };
    let (driven, sites) = super::login_of(&login("https://auth.example.test")).unwrap();
    assert_eq!(driven.origin, "https://auth.example.test");
    assert_eq!(driven.back_to.as_deref(), Some("https://app.example.test"));
    assert_eq!(
        sites,
        LoginSites {
            page: Site::of("app.example.test", 443),
            form: Some(Site::of("auth.example.test", 443)),
        }
    );
    let (same, sites) = super::login_of(&login("https://app.example.test")).unwrap();
    assert_eq!(same.back_to, None);
    assert_eq!(sites, LoginSites::page(Site::of("app.example.test", 443)));
    for refused in [
        "http://auth.example.test",
        "https://auth.example.test/realm",
    ] {
        assert!(super::login_of(&login(refused)).is_err(), "{refused}");
    }
}

/// #6291 (the operator's ruling of 2026-11-12): a login's sign-in starts where the chat named it,
/// its query as written, while it fills on its form's origin and is held to its page's site and
/// its form's as before. Judged again by the host, whatever the window sent.
#[test]
fn a_login_with_a_start_opens_it_as_written_and_is_held_to_its_sites_as_before() {
    use purlis_host::protocol::browsing::{LogIn, Secret};
    let start = "https://auth.example.test/authorize?client_id=app\
                 &redirect_uri=https%3A%2F%2Fapp.example.test%2Fcb&state=x%2By#f";
    let login = |start: &str| LogIn {
        address: "https://app.example.test/".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
        identity: None,
        suggested: false,
        form: Some("https://auth.example.test".into()),
        start: Some(start.into()),
    };
    let (driven, sites) = super::login_of(&login(start)).unwrap();
    assert_eq!(driven.address.as_str(), start);
    assert_eq!(driven.origin, "https://auth.example.test");
    assert_eq!(driven.back_to.as_deref(), Some("https://app.example.test"));
    assert_eq!(
        sites,
        LoginSites {
            page: Site::of("app.example.test", 443),
            form: Some(Site::of("auth.example.test", 443)),
        }
    );
    for refused in [
        "https://elsewhere.example.test/authorize",
        "https://auth.example.test/authorize?redirect_uri=https%3A%2F%2Fevil.test%2Fcb",
    ] {
        assert!(super::login_of(&login(refused)).is_err(), "{refused}");
    }
}

/// A login performed through a real reach proxy and judge (#2016): the run is handed what binds
/// the browser, and once it has, another site its chat lists is refused, the login's own is
/// carried, and a second login at another site is refused. These bind sockets, so they first
/// run on CI.
#[cfg(unix)]
#[test]
fn a_login_binds_the_real_reach_proxy_to_its_site_alone() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    use purlis_browser_engine::drive::Hold;

    use crate::sandbox::browser_reach::{Lane, ReachProxy};

    let server = || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
        let port = listener.local_addr().expect("an address").port();
        (listener, port)
    };
    let (_login_site, login_port) = server();
    let (_other_site, other_port) = server();
    let reach = BrowserReach {
        listed: [login_port, other_port]
            .iter()
            .flat_map(|port| crate::sandbox::localport::reach_entries(*port))
            .map(|host| Listed {
                host,
                layer: Layer::Chat,
            })
            .collect(),
        ..BrowserReach::default()
    };
    let sites = std::sync::Arc::new(super::judge::SiteJudge::of(&reach).unwrap());
    let (listed, _) = reach_of(Some(&reach)).unwrap();
    let proxy = std::sync::Arc::new(
        ReachProxy::bound_by(Lane::of(listed), sites.new_sites(), sites.bound().clone()).unwrap(),
    );
    let status = |port: u16| -> String {
        let mut stream = TcpStream::connect(proxy.address()).expect("the proxy");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream
            .write_all(format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        line.trim_end().to_owned()
    };
    assert_eq!(status(other_port), "HTTP/1.1 200 Connection established");
    let login_site = LoginSites::page(Site::of("127.0.0.1", login_port));
    let bound = super::performed(
        &sites,
        &proxy,
        &login_site,
        "http://127.0.0.1",
        super::SIGN_IN_WITHIN,
        |holder| holder.bind(),
    )
    .expect("performed");
    assert_eq!(bound, Ok(()));
    assert_eq!(status(other_port), "HTTP/1.1 403 Forbidden");
    assert_eq!(status(login_port), "HTTP/1.1 200 Connection established");
    let ran = std::cell::Cell::new(false);
    let refused = super::performed(
        &sites,
        &proxy,
        &LoginSites::page(Site::of("127.0.0.1", other_port)),
        "http://127.0.0.1",
        super::SIGN_IN_WITHIN,
        |_| ran.set(true),
    )
    .unwrap_err();
    assert!(refused.contains("holds a login at 127.0.0.1:"), "{refused}");
    assert!(!ran.get(), "a second login at another site runs nothing");
}

/// #5117: while a page holding a secret purlis filled holds the browser, an address the agent
/// writes for another site is refused before the driver hears of it, saying where the browser is
/// held and what ends it; one on the held site, and every other act, go.
#[test]
fn an_address_for_another_site_is_refused_with_why_while_a_filled_page_holds_the_browser() {
    use crate::sandbox::browser_reach::Bound;
    let navigate = |address: &str| Act::Navigate {
        page: None,
        address: address.into(),
    };
    let bound = Bound::default();
    assert_eq!(
        super::held_to_said(&bound, &navigate("https://elsewhere.example.test/")),
        None,
        "nothing holds it yet"
    );
    bound
        .hold_for_fill(&Site::of("dash.example.test", 443))
        .expect("held");

    let said =
        super::held_to_said(&bound, &navigate("https://elsewhere.example.test/")).expect("refused");
    assert!(
        said.contains("reaches only dash.example.test:443"),
        "{said}"
    );
    assert!(said.contains("elsewhere.example.test:443"), "{said}");
    assert!(said.contains("browser_close"), "{said}");
    for went in [
        navigate("https://dash.example.test/settings"),
        Act::Changes { page: None },
    ] {
        assert_eq!(super::held_to_said(&bound, &went), None, "{went:?}");
    }
    for refused in [
        Act::Open {
            address: "http://dash.example.test/".into(),
        },
        Act::FindLoginForm {
            address: "https://elsewhere.example.test/login".into(),
        },
    ] {
        assert!(
            super::held_to_said(&bound, &refused).is_some(),
            "{refused:?}"
        );
    }
}

/// #5150: the engine's own page for a load the hold refused answers why, naming the site the
/// browser is held to and what ends the hold, and nothing of the address, which may carry what a
/// filled page sent.
#[test]
fn a_load_a_fills_hold_refused_says_why_and_nothing_of_where_it_went() {
    use crate::sandbox::browser_reach::Bound;
    let refused = "https://elsewhere.example.test/search?q=open-sesame";
    let bound = Bound::default();
    assert_eq!(
        super::load_held_said(&bound, refused),
        None,
        "nothing holds"
    );
    bound
        .hold_for_fill(&Site::of("dash.example.test", 443))
        .expect("held");

    let said = super::load_held_said(&bound, refused).expect("refused");

    assert!(
        said.contains("reaches only dash.example.test:443"),
        "{said}"
    );
    assert!(said.contains("pages of other sites do not load"), "{said}");
    assert!(said.contains("browser_close"), "{said}");
    for never in ["elsewhere", "open-sesame"] {
        assert!(!said.contains(never), "{never}: {said}");
    }
    assert_eq!(
        super::load_held_said(&bound, "https://dash.example.test/gone"),
        None,
        "a page of the site it is held to failed by itself"
    );
    assert_eq!(super::load_held_said(&bound, "about:blank"), None);
}

/// #5117: a fill's holder holds a real reach proxy to the field's site from its hold until the
/// engine says the page is gone; another site the chat lists is refused meanwhile. These bind
/// sockets, so they first run on CI.
#[cfg(unix)]
#[test]
fn a_fill_holds_the_real_reach_proxy_to_its_site_until_its_page_is_gone() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    use purlis_browser_engine::drive::FillHold;

    use crate::sandbox::browser_reach::{Lane, ReachProxy};

    let server = || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
        let port = listener.local_addr().expect("an address").port();
        (listener, port)
    };
    let (_held_site, held_port) = server();
    let (_other_site, other_port) = server();
    let reach = BrowserReach {
        listed: [held_port, other_port]
            .iter()
            .flat_map(|port| crate::sandbox::localport::reach_entries(*port))
            .map(|host| Listed {
                host,
                layer: Layer::Chat,
            })
            .collect(),
        ..BrowserReach::default()
    };
    let sites = std::sync::Arc::new(super::judge::SiteJudge::of(&reach).unwrap());
    let (listed, _) = reach_of(Some(&reach)).unwrap();
    let proxy = std::sync::Arc::new(
        ReachProxy::bound_by(Lane::of(listed), sites.new_sites(), sites.bound().clone()).unwrap(),
    );
    let status = |port: u16| -> String {
        let mut stream = TcpStream::connect(proxy.address()).expect("the proxy");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream
            .write_all(format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        line.trim_end().to_owned()
    };
    let carried = "HTTP/1.1 200 Connection established";
    let holder =
        super::FillHolder::of(&proxy, &format!("http://127.0.0.1:{held_port}")).expect("a site");
    assert_eq!(status(other_port), carried);

    holder.hold().expect("held");

    assert_eq!(status(other_port), "HTTP/1.1 403 Forbidden");
    assert_eq!(status(held_port), carried);
    holder.released();
    // Held a while yet: the page that left may still send as it unloads.
    assert_eq!(status(other_port), "HTTP/1.1 403 Forbidden");
    std::thread::sleep(super::FILL_RELEASED_AFTER + Duration::from_millis(500));
    assert_eq!(status(other_port), carried, "the browser reaches as before");
    holder.released();
    assert_eq!(status(other_port), carried);
}

/// #5230: a fill's holder asks the browser's judge where its page may leave for, is approved for
/// the sites it was given, and holds the proxy to them too. These bind sockets, so run on CI.
#[cfg(unix)]
#[test]
fn a_fill_holds_its_other_approved_sites_too_and_its_judge_says_where_it_may_leave_for() {
    use purlis_browser_engine::drive::{FillHold, Leaving};

    use crate::sandbox::browser_reach::{Lane, ReachProxy};

    let reach = BrowserReach {
        open_web: true,
        ..BrowserReach::default()
    };
    let sites = std::sync::Arc::new(super::judge::SiteJudge::of(&reach).unwrap());
    let (listed, _) = reach_of(Some(&reach)).unwrap();
    let proxy = std::sync::Arc::new(
        ReachProxy::bound_by(Lane::of(listed), sites.new_sites(), sites.bound().clone()).unwrap(),
    );
    let holder = super::FillHolder::of(&proxy, "https://auth.example.com")
        .expect("a site")
        .judged_by(Some(&sites), &["https://sso.example.com".to_owned()]);

    holder.hold().expect("held");

    assert!(sites.bound().lets("sso.example.com", 443));
    assert!(!sites.bound().lets("elsewhere.example.com", 443));
    assert!(holder.approved_for("https://sso.example.com/in"));
    assert!(!holder.approved_for("https://elsewhere.example.com/in"));
    assert!(!holder.approved_for("http://sso.example.com:443/in"));
    assert_eq!(
        holder.may_leave_for("https://app.example.com/?code=1", true),
        Leaving::Go
    );
    assert!(matches!(
        holder.may_leave_for("https://new.example.com/", false),
        Leaving::Ask { .. }
    ));
    let unjudged = super::FillHolder::of(&proxy, "https://auth.example.com").expect("a site");
    assert!(matches!(
        unjudged.may_leave_for("https://app.example.com/", true),
        Leaving::No { .. }
    ));
}

/// #5121: a login that needs another site than the one a page holding a filled secret holds the
/// browser to runs nothing and says why, and one of that very site goes ahead.
#[cfg(unix)]
#[test]
fn a_login_of_another_site_is_refused_while_a_page_holding_a_secret_holds_the_browser() {
    use crate::sandbox::browser_reach::{Lane, LoginSites, ReachProxy, Site};

    let reach = BrowserReach::default();
    let sites = std::sync::Arc::new(super::judge::SiteJudge::of(&reach).unwrap());
    let (listed, _) = reach_of(Some(&reach)).unwrap();
    let proxy = std::sync::Arc::new(
        ReachProxy::bound_by(Lane::of(listed), sites.new_sites(), sites.bound().clone()).unwrap(),
    );
    let held = Site::of("dash.example.test", 443);
    proxy.hold_for_fill(&held).expect("held");
    let run = |at: &LoginSites| {
        let mut ran = false;
        let done = super::performed(
            &sites,
            &proxy,
            at,
            "https://any.example.test",
            Duration::from_secs(60),
            |_| ran = true,
        );
        (done, ran)
    };

    let (done, ran) = run(&LoginSites::page(Site::of("shop.example.test", 443)));
    assert!(!ran, "nothing ran");
    let why = done.expect_err("refused");
    assert!(why.contains("dash.example.test"), "{why}");
    assert!(why.contains("nothing was filled"), "{why}");

    let (done, ran) = run(&LoginSites::page(held));
    assert!(done.is_ok() && ran, "that very site goes ahead");
}

/// #5272: only a browser with nothing behind it skips the clearing: an act of anyone's, a login tried
/// or a kept login it opened from is something to clear.
#[test]
fn only_a_browser_with_nothing_behind_it_has_held_nothing() {
    let home = tempfile::tempdir().unwrap();
    let new = || super::Keeping::new(home.path(), home.path());
    assert!(new().held_nothing());
    let acted = new();
    acted.browsed();
    assert!(!acted.held_nothing(), "an act");
    let mut tried = new();
    tried.tried(None);
    assert!(!tried.held_nothing(), "a login tried");
    let mut opened = new();
    opened.opened_from = Some(wire::KeptLogin {
        form: None,
        profile: "p".into(),
        persona: "q".into(),
        login: "l".into(),
        identity: "i".into(),
        address: "https://shop.example.test/".into(),
    });
    assert!(!opened.held_nothing(), "a kept login");
}

/// #5272: what a host hands the engine is never "clean already" by itself: it's found out.
#[test]
fn a_logins_address_says_nothing_of_whether_it_is_clean_already() {
    let (login, _) = super::login_of(&wire::LogIn {
        address: "https://shop.example.test/login".to_owned(),
        user: None,
        password: wire::Secret::new("word".to_owned()),
        keep: None,
        identity: None,
        suggested: false,
        form: None,
        start: None,
    })
    .unwrap();
    assert!(!login.clean_already);
}

/// #5121: a login counts as signed in, for a later call, only where it answered with the browser
/// held to its page.
#[test]
fn a_login_counts_as_signed_in_only_where_it_answered_and_holds_the_browser_to_its_page() {
    use crate::sandbox::browser_reach::Site;
    let page = Site::of("shop.example.test", 443);
    let other = Site::of("other.example.test", 443);
    assert!(super::counts_as_signed_in(true, Some(&page), &page));
    assert!(
        !super::counts_as_signed_in(false, Some(&page), &page),
        "it failed"
    );
    assert!(
        !super::counts_as_signed_in(true, None, &page),
        "nothing bound"
    );
    assert!(
        !super::counts_as_signed_in(true, Some(&other), &page),
        "another's"
    );
}

/// #5121: only a login that went through counts: one merely tried may have bound nothing.
#[test]
fn a_browser_knows_only_the_logins_that_went_through_in_it_by_their_identities() {
    let home = tempfile::tempdir().unwrap();
    let mut keeping = super::Keeping::new(home.path(), home.path());
    keeping.tried(None);
    assert!(!keeping.went_through(Some("one")), "tried, no more");
    keeping.goes_through(None);
    assert!(!keeping.went_through(None), "no identity: nothing to know");
    keeping.goes_through(Some("one"));
    assert!(keeping.went_through(Some("one")));
    assert!(!keeping.went_through(Some("two")), "another login");
    keeping.only_goes_through(Some("two"));
    assert!(
        !keeping.went_through(Some("one")),
        "another login ran since"
    );
    keeping.goes_through(Some("two"));
    keeping.did_not_hold(Some("two"));
    assert!(!keeping.went_through(Some("two")), "it failed since");
}

/// #5152, the person's ruling of 2026-11-04 (amending #4995's of 2026-11-03): a login whose form
/// is on another site, performed through a real reach proxy and judge, reaches the form's site
/// from its bind for the browser's life, and keeps the agent off it once its page is back on the
/// page's origin. One whose page never comes back keeps the agent off it once its sign-in's time
/// is up. These bind sockets, so they first run on CI.
#[cfg(unix)]
#[test]
fn a_login_whose_form_is_elsewhere_reaches_it_and_keeps_the_agent_off_it_once_signed_in() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    use purlis_browser_engine::drive::Hold;

    use crate::sandbox::browser_reach::{Lane, ReachProxy};

    let server = || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
        let port = listener.local_addr().expect("an address").port();
        (listener, port)
    };
    let (_page_site, page_port) = server();
    let (_form_site, form_port) = server();
    let reach = BrowserReach {
        listed: [page_port, form_port]
            .iter()
            .flat_map(|port| crate::sandbox::localport::reach_entries(*port))
            .map(|host| Listed {
                host,
                layer: Layer::Chat,
            })
            .collect(),
        ..BrowserReach::default()
    };
    let login = LoginSites {
        page: Site::of("127.0.0.1", page_port),
        form: Some(Site::of("127.0.0.1", form_port)),
    };
    let a_browser = || {
        let sites = std::sync::Arc::new(super::judge::SiteJudge::of(&reach).unwrap());
        let (listed, _) = reach_of(Some(&reach)).unwrap();
        let proxy = std::sync::Arc::new(
            ReachProxy::bound_by(Lane::of(listed), sites.new_sites(), sites.bound().clone())
                .unwrap(),
        );
        (sites, proxy)
    };
    let status = |proxy: &ReachProxy, port: u16| -> String {
        let mut stream = TcpStream::connect(proxy.address()).expect("the proxy");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream
            .write_all(format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        line.trim_end().to_owned()
    };
    let carried = "HTTP/1.1 200 Connection established";
    let refused = "HTTP/1.1 403 Forbidden";
    let form_address = format!("http://127.0.0.1:{form_port}/account");
    let kept_off =
        |sites: &super::judge::SiteJudge| super::kept_off_at(sites.bound(), &form_address);
    let form_site = Some(format!("127.0.0.1:{form_port}"));

    // Its page comes back.
    let (sites, proxy) = a_browser();
    let holder = super::performed(
        &sites,
        &proxy,
        &login,
        "http://127.0.0.1",
        super::SIGN_IN_WITHIN,
        |holder| {
            holder.bind().expect("bound");
            std::sync::Arc::clone(holder)
        },
    )
    .expect("performed");
    assert_eq!(status(&proxy, form_port), carried, "while it signs in");
    assert_eq!(kept_off(&sites), None, "while it signs in");
    assert_eq!(holder.signing_in().map(|(form, _)| form), login.form);
    holder.back();
    assert_eq!(status(&proxy, form_port), carried, "back on its page");
    assert_eq!(kept_off(&sites), form_site, "back on its page");
    assert_eq!(status(&proxy, page_port), carried);
    assert_eq!(holder.signing_in(), None);
    // A later login of the same page signs in anew.
    let again = super::performed(
        &sites,
        &proxy,
        &login,
        "http://127.0.0.1",
        super::SIGN_IN_WITHIN,
        |holder| {
            holder.bind().expect("bound again");
            std::sync::Arc::clone(holder)
        },
    )
    .expect("performed again");
    assert_eq!(kept_off(&sites), None, "a later login signs in anew");
    holder.back();
    assert_eq!(kept_off(&sites), None, "an earlier login ends no later one");
    again.sign_in_over();
    assert_eq!(kept_off(&sites), form_site);
    assert_eq!(status(&proxy, form_port), carried);

    // Its page never comes back.
    let (sites, proxy) = a_browser();
    let _holder = super::performed(
        &sites,
        &proxy,
        &login,
        "http://127.0.0.1",
        Duration::from_millis(300),
        |holder| {
            holder.bind().expect("bound");
            std::sync::Arc::clone(holder)
        },
    )
    .expect("performed");
    assert_eq!(kept_off(&sites), None, "while it signs in");
    let start = std::time::Instant::now();
    while kept_off(&sites).is_none() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the sign-in outlived its time"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(status(&proxy, form_port), carried);
    assert_eq!(status(&proxy, page_port), carried);

    // A browser held to the login's page already, as one opened from kept logins is, reaches
    // the form's site from the moment the login begins; a login that answers there with nothing
    // filled ends its sign-in, and the browser goes on reaching it.
    let (sites, proxy) = a_browser();
    proxy
        .bind(&LoginSites::page(login.page.clone()))
        .expect("held to its page");
    assert_eq!(status(&proxy, form_port), refused, "held to its page alone");
    let holder = super::performed(
        &sites,
        &proxy,
        &login,
        "http://127.0.0.1",
        super::SIGN_IN_WITHIN,
        |holder| {
            holder.begins().expect("begun");
            std::sync::Arc::clone(holder)
        },
    )
    .expect("performed");
    assert_eq!(status(&proxy, form_port), carried, "once the login begins");
    assert_eq!(kept_off(&sites), None, "once the login begins");
    holder.answered(&Ok(drive::LoginEnded {
        driver: drive::Driver::Agent,
        epoch: 1,
        outcome: Ok(Answer {
            page: None,
            settled: Settled::Yes,
            opened: Vec::new(),
            content: Content::Nothing,
            note: None,
            repeated: false,
            asked: Vec::new(),
            downloads: Vec::new(),
        }),
    }));
    assert_eq!(
        kept_off(&sites),
        form_site,
        "signed in already, nothing was filled"
    );
    assert_eq!(status(&proxy, form_port), carried);
    // One that binds and fails with the browser handed back to the agent ends it too; one the
    // person took over goes on.
    let failed = |driver| drive::LoginEnded {
        driver,
        epoch: 2,
        outcome: Err(drive::Refusal::NotLoggedIn("no form".into())),
    };
    for (driver, after) in [
        (drive::Driver::Person, None),
        (drive::Driver::Agent, form_site.clone()),
    ] {
        let holder = super::performed(
            &sites,
            &proxy,
            &login,
            "http://127.0.0.1",
            super::SIGN_IN_WITHIN,
            |holder| {
                holder.begins().expect("begun");
                holder.bind().expect("bound");
                std::sync::Arc::clone(holder)
            },
        )
        .expect("performed");
        holder.answered(&Ok(failed(driver)));
        assert_eq!(kept_off(&sites), after, "{driver:?}");
        assert_eq!(status(&proxy, form_port), carried, "{driver:?}");
        holder.sign_in_over();
    }
}

// ---- Kept logins (#2021) ----

fn kept_login() -> wire::KeptLogin {
    wire::KeptLogin {
        form: None,
        profile: "claude".into(),
        persona: "steward".into(),
        login: "shop-login".into(),
        identity: "0123456789abcdef0123456789abcdef".into(),
        address: "https://shop.example.test/login".into(),
    }
}

/// A browser's keeping, on a keyring of the test's own.
fn keeping(
    home: &Path,
    folder: &Path,
    keyring: &std::sync::Arc<crate::secrets::keyring::FileStore>,
) -> super::Keeping {
    let mut keeping = super::Keeping::new(home, folder);
    keeping.keyring = keyring.clone();
    keeping.device = Some("01JDEVICE0000000000000000".into());
    keeping
}

fn signed_in(folder: &Path) {
    std::fs::create_dir_all(folder.join("Default")).unwrap();
    std::fs::write(folder.join("Default/Cookies"), "session=fixture-cookie").unwrap();
}

#[test]
fn a_browser_that_logged_in_keeps_its_logins_and_the_next_opens_signed_in() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let first = browsers_in(&home).join("first");
    signed_in(&first);
    let mut it = keeping(&home, &first, &keyring);
    assert_eq!(it.keep(&kept_login()), None);
    it.write_back();
    drop(it);
    assert!(super::store_of(&kept_login()).unwrap().exists(&home));
    let next = browsers_in(&home).join("next");
    let mut opened = keeping(&home, &next, &keyring);
    opened.open(&kept_login()).unwrap();
    assert_eq!(
        std::fs::read_to_string(next.join("Default/Cookies")).unwrap(),
        "session=fixture-cookie"
    );
    assert_eq!(opened.opened_from, Some(kept_login()));
    assert!(opened.writes.is_some(), "it writes them back");
    // A second browser at once gets a copy, and is told so.
    let second = browsers_in(&home).join("second");
    let mut copy = keeping(&home, &second, &keyring);
    copy.open(&kept_login()).unwrap();
    assert!(copy.writes.is_none());
    let said = copy.keep(&kept_login()).unwrap();
    assert!(said.contains("opened a copy"), "{said}");
}

#[test]
fn a_folder_an_earlier_kept_browser_left_behind_is_swept_and_a_running_ones_is_not() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let running = browsers_in(&home).join("running");
    signed_in(&running);
    let mut live = keeping(&home, &running, &keyring);
    assert_eq!(live.keep(&kept_login()), None);
    let left = browsers_in(&home).join("left");
    signed_in(&left);
    drop(crate::browserlogins::mark(&left, &super::store_of(&kept_login()).unwrap()).unwrap());
    let next = browsers_in(&home).join("next");
    keeping(&home, &next, &keyring).open(&kept_login()).unwrap();
    assert!(!left.exists(), "left behind, swept");
    assert!(
        running.join("Default/Cookies").exists(),
        "a running browser's is left alone"
    );
}

#[test]
fn a_browser_opened_from_one_login_keeps_no_other() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("one");
    let mut it = keeping(&home, &folder, &keyring);
    it.open(&kept_login()).unwrap();
    let other = wire::KeptLogin {
        form: None,
        login: "mail-login".into(),
        ..kept_login()
    };
    let said = it.keep(&other).unwrap();
    assert!(said.contains("not kept"), "{said}");
    assert!(it.writes.as_ref().is_some_and(|(w, _)| *w == kept_login()));
}

#[test]
fn a_browser_that_went_elsewhere_before_its_first_login_is_not_kept_as_that_logins_store() {
    // #2693: what it holds may be another session of the login's site, or another site's.
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("browsed");
    signed_in(&folder);
    let mut it = keeping(&home, &folder, &keyring);
    it.browsed();

    let said = it.keep(&kept_login()).expect("it is told");

    assert!(said.contains("not kept"), "{said}");
    assert!(said.contains("browser_new"), "{said}");
    assert!(it.writes.is_none());
    it.write_back();
    assert!(!super::store_of(&kept_login()).unwrap().exists(&home));
}

#[test]
fn a_browser_in_which_another_login_was_tried_keeps_no_later_one() {
    // #2693: the first login's session may be in it, though its store was not written.
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let first = kept_login();
    let second = wire::KeptLogin {
        form: None,
        login: "mail-login".into(),
        ..kept_login()
    };
    // The first login's store is held by another browser: this one keeps none of it.
    let _elsewhere = crate::browserlogins::hold(&home, &super::store_of(&first).unwrap())
        .unwrap()
        .expect("held elsewhere");
    let folder = browsers_in(&home).join("one");
    signed_in(&folder);
    let mut it = keeping(&home, &folder, &keyring);
    it.tried(Some(&first));
    assert!(it.keep(&first).is_some(), "held elsewhere");

    it.tried(Some(&second));
    let said = it.keep(&second).expect("it is told");

    assert!(said.contains("not kept"), "{said}");
    assert!(it.writes.is_none());
    it.write_back();
    assert!(!super::store_of(&second).unwrap().exists(&home));
}

#[test]
fn a_browser_holding_one_logins_store_writes_nothing_back_once_another_login_is_tried() {
    // #2693: the other login's session would be sealed in as the first one's.
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let first = kept_login();
    let other = wire::KeptLogin {
        form: None,
        login: "mail-login".into(),
        ..kept_login()
    };
    let store = super::store_of(&first).unwrap();
    // The first login kept once, as last written.
    let earlier = browsers_in(&home).join("earlier");
    signed_in(&earlier);
    let mut kept = keeping(&home, &earlier, &keyring);
    kept.tried(Some(&first));
    assert_eq!(kept.keep(&first), None);
    kept.write_back();
    drop(kept);
    let sealed = std::fs::read(store.dir(&home).join(crate::browserlogins::FILE)).unwrap();
    // A browser opened from it, in which another login is then tried.
    let folder = browsers_in(&home).join("opened");
    let mut it = keeping(&home, &folder, &keyring);
    it.open(&first).unwrap();
    it.tried(Some(&first));
    assert_eq!(it.keep(&first), None);
    std::fs::write(folder.join("Default/Cookies"), "session=the-other-login").unwrap();

    it.tried(Some(&other));
    let said = it.keep(&other).expect("it is told");
    it.write_back();

    assert!(said.contains("not kept"), "{said}");
    assert!(it.writes.is_none(), "it holds the store no more");
    assert_eq!(
        std::fs::read(store.dir(&home).join(crate::browserlogins::FILE)).unwrap(),
        sealed,
        "the first login's store is as last written"
    );
    assert!(!super::store_of(&other).unwrap().exists(&home));
}

#[test]
fn the_same_login_tried_again_is_still_kept() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("one");
    signed_in(&folder);
    let mut it = keeping(&home, &folder, &keyring);
    it.tried(Some(&kept_login()));
    it.tried(Some(&kept_login()));

    assert_eq!(it.keep(&kept_login()), None);
}

#[test]
fn a_browser_in_which_a_login_that_keeps_nothing_was_tried_keeps_no_later_one() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("one");
    signed_in(&folder);
    let mut it = keeping(&home, &folder, &keyring);
    it.tried(None);
    it.tried(Some(&kept_login()));

    assert!(it.keep(&kept_login()).is_some());
    assert!(it.writes.is_none());
}

#[test]
fn a_browser_opened_from_its_store_keeps_it_however_it_browsed_since() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("opened");
    let mut it = keeping(&home, &folder, &keyring);
    it.open(&kept_login()).unwrap();
    it.browsed();

    assert_eq!(it.keep(&kept_login()), None);
    assert!(it.writes.is_some());
}

/// #5272: a browser whose login moved on writes nothing it cleared over its store, and leaves it for
/// the new browser to keep.
#[test]
fn a_browser_whose_form_did_not_appear_on_its_clean_load_lets_go_of_its_store() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("opened");
    let mut it = keeping(&home, &folder, &keyring);
    it.open(&kept_login()).unwrap();
    assert_eq!(it.keep(&kept_login()), None);
    assert!(it.writes.is_some());

    it.no_form_on_clean_load();

    assert!(it.writes.is_none());
    let next = browsers_in(&home).join("next");
    let mut clean = keeping(&home, &next, &keyring);
    clean.tried(Some(&kept_login()));
    assert_eq!(clean.keep(&kept_login()), None, "the new browser keeps it");
    assert!(clean.writes.is_some());
}

#[test]
fn a_removed_profiles_running_browser_does_not_make_its_store_again() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("running");
    signed_in(&folder);
    let mut it = keeping(&home, &folder, &keyring);
    assert_eq!(it.keep(&kept_login()), None);
    let keys = crate::browserlogins::Keys {
        store: &*keyring,
        device: "01JDEVICE0000000000000000".into(),
    };
    crate::browserlogins::wipe_profile(&home, &browsers_in(&home), "claude", &keys).unwrap();
    it.write_back();
    assert!(!super::store_of(&kept_login()).unwrap().exists(&home));
    assert!(
        !home.join(crate::browserlogins::DIR).join("claude").exists(),
        "nothing of the profile made again"
    );
}

#[test]
fn a_browser_whose_profile_was_removed_before_its_first_login_makes_no_store() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("data");
    let keyring = std::sync::Arc::new(crate::secrets::keyring::FileStore::at(
        base.path().join("keyring.json"),
    ));
    let folder = browsers_in(&home).join("running");
    signed_in(&folder);
    let mut it = keeping(&home, &folder, &keyring);
    crate::browserlogins::removed(&home, "claude").unwrap();

    assert!(it.keep(&kept_login()).is_some(), "it is told");

    assert!(it.writes.is_none());
    assert!(
        !home.join(crate::browserlogins::DIR).join("claude").exists(),
        "no folder of the removed profile made again"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn the_browsers_folder_is_made_private_and_left_out_of_this_macs_backups() {
    let base = tempfile::tempdir().unwrap();
    let folders = browsers_in(&base.path().join("data"));
    super::made_private_and_unbacked(&folders).unwrap();
    let set = xattr::get(&folders, crate::browserlogins::BACKUP_EXCLUSION)
        .unwrap()
        .expect("excluded");
    assert_eq!(set, crate::browserlogins::BACKUP_EXCLUDED);
    use std::os::unix::fs::PermissionsExt as _;
    let mode = std::fs::metadata(&folders).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn a_browsers_folder_left_looser_by_an_earlier_version_is_tightened_and_a_link_is_refused() {
    use std::os::unix::fs::PermissionsExt as _;
    let base = tempfile::tempdir().unwrap();
    let folders = browsers_in(&base.path().join("data"));
    std::fs::create_dir_all(folders.join("01JEARLIER")).unwrap();
    std::fs::set_permissions(&folders, std::fs::Permissions::from_mode(0o755)).unwrap();
    super::made_private_and_unbacked(&folders).expect("tightened, not refused");
    let mode = std::fs::metadata(&folders).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert!(folders.join("01JEARLIER").is_dir(), "what was there stays");
    // A link in its place is refused, and nothing is made where it points.
    let elsewhere = base.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let linked = browsers_in(&base.path().join("linked"));
    std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
    assert!(super::made_private_and_unbacked(&linked).is_err());
    let mode = std::fs::metadata(&elsewhere).unwrap().permissions().mode();
    assert_ne!(mode & 0o777, 0o700, "a link's target is left as it was");
}

#[test]
fn a_browsers_engine_log_is_beside_its_folder_so_the_folders_removal_leaves_it() {
    let folder = Path::new("/home/me/.local/share/purlis/browsers/01K2ABC");
    assert_eq!(
        super::engine_log_of(folder),
        Path::new("/home/me/.local/share/purlis/browsers/01K2ABC.engine.log")
    );
}

#[test]
fn the_newest_engine_logs_are_kept_and_a_running_browsers_never_goes() {
    let folders = tempfile::tempdir().unwrap();
    let at = |n: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000 + n);
    for n in 0..5u64 {
        let log = folders.path().join(format!("b{n}.engine.log"));
        let file = std::fs::File::create(&log).unwrap();
        file.set_modified(at(n)).unwrap();
    }
    // The oldest is of a browser still running: its folder is there.
    std::fs::create_dir(folders.path().join("b0")).unwrap();
    // Nothing else in the folder is a log.
    std::fs::write(folders.path().join("b9.kept"), "").unwrap();

    super::sweep_engine_logs(folders.path(), 2);

    let mut left: Vec<String> = std::fs::read_dir(folders.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "b0",
            "b0.engine.log",
            "b3.engine.log",
            "b4.engine.log",
            "b9.kept"
        ]
    );
}

#[test]
fn a_browsers_end_is_told_with_its_exit_why_and_where_its_engines_log_is() {
    use purlis_browser_engine::cdp::Closed;
    use purlis_browser_engine::ending::{EndedBy, Ending, EngineExit};
    use purlis_host::protocol::session::Exit;

    let log = Path::new("/data/browsers/01K2.engine.log");
    let ended = super::ended_of(
        &Ending {
            by: EndedBy::LostConnection,
            exit: Some(EngineExit::Signal("SIGKILL".into())),
            closed: Some(Closed::NotDevTools {
                method: Some("Target.targetInfoChanged".into()),
            }),
        },
        log,
    );
    assert_eq!(ended.exit, Exit::Signal("SIGKILL".into()));
    assert_eq!(
        ended.why,
        "purlis ended its engine (signal SIGKILL) because its DevTools connection closed: the \
         engine sent a message (Target.targetInfoChanged) that does not read as DevTools JSON; \
         its engine's log is /data/browsers/01K2.engine.log"
    );
    let stopped = super::ended_of(
        &Ending {
            by: EndedBy::Stopped,
            exit: Some(EngineExit::Code(0)),
            closed: None,
        },
        log,
    );
    assert_eq!(stopped.exit, Exit::Code(0));
}

// ---- Which engine a browser starts on (#2835) ----

use purlis_browser_engine::install::EngineChoice;

use super::{Chosen, engine_for};

const CHROME: &str = "/usr/bin/google-chrome";

#[test]
fn the_pinned_engine_starts_where_it_can_make_its_sandbox() {
    let chosen = Chosen {
        machine_chrome: false,
        found: Some(CHROME.into()),
        sandboxes: true,
    };
    assert_eq!(engine_for(&chosen), Ok(EngineChoice::Pinned));
}

#[test]
fn where_it_cannot_and_nothing_else_is_chosen_the_start_is_refused_in_words() {
    let chosen = Chosen {
        machine_chrome: false,
        found: Some(CHROME.into()),
        sandboxes: false,
    };
    assert_eq!(
        engine_for(&chosen),
        Err(super::sandboxing::NO_ENGINE_SANDBOX.to_owned()),
        "never the engine without its sandbox"
    );
}

#[test]
fn this_machines_chrome_starts_once_the_person_chose_it() {
    let chosen = Chosen {
        machine_chrome: true,
        found: Some(CHROME.into()),
        sandboxes: false,
    };
    assert_eq!(
        engine_for(&chosen),
        Ok(EngineChoice::Machine(CHROME.into()))
    );
}

#[test]
fn a_chosen_chrome_that_is_gone_leaves_the_pinned_engine_as_it_stands() {
    let gone = |sandboxes| Chosen {
        machine_chrome: true,
        found: None,
        sandboxes,
    };
    assert_eq!(engine_for(&gone(true)), Ok(EngineChoice::Pinned));
    assert_eq!(
        engine_for(&gone(false)),
        Err(super::sandboxing::NO_ENGINE_SANDBOX.to_owned())
    );
}

// ---- The engine's first download, told and cancelled (#2207) ----

use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

use super::{DOWNLOAD_TOLD_EVERY, Progressing, download_failed_said};

/// An archive of `chunks` reads of `size` bytes each, whose length is said.
struct Chunked {
    chunks: usize,
    size: usize,
}

impl Fetch for Chunked {
    fn fetch(&self, url: &str) -> std::io::Result<Box<dyn Read + '_>> {
        self.fetch_sized(url).map(|(body, _)| body)
    }

    fn fetch_sized(&self, _url: &str) -> std::io::Result<(Box<dyn Read + '_>, Option<u64>)> {
        let total = self.chunks * self.size;
        Ok((
            Box::new(OneReadAtATime {
                body: Cursor::new(vec![7_u8; total]),
                size: self.size,
            }),
            Some(total as u64),
        ))
    }
}

/// A body that gives at most `size` bytes a read.
struct OneReadAtATime {
    body: Cursor<Vec<u8>>,
    size: usize,
}

impl Read for OneReadAtATime {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let most = buf.len().min(self.size);
        self.body.read(&mut buf[..most])
    }
}

/// What a download was told, in order: how far it had come, and of how much.
type Told = Arc<Mutex<Vec<(u64, Option<u64>)>>>;

fn told() -> (purlis_host::Download, Told) {
    let told: Told = Arc::default();
    let into = Arc::clone(&told);
    (
        purlis_host::Download::telling(Arc::new(move |done, total| {
            into.lock().unwrap().push((done, total));
        })),
        told,
    )
}

#[test]
fn a_download_is_told_a_few_times_a_second_at_most_and_once_as_it_ends() {
    let (download, told) = told();
    let archive = Chunked {
        chunks: 50,
        size: 1000,
    };
    let reads = Progressing::new(&archive, &download, Duration::from_secs(3600));

    let mut body = reads.fetch("https://engine.test/a.zip").unwrap();
    std::io::copy(&mut body, &mut std::io::sink()).unwrap();

    assert_eq!(
        *told.lock().unwrap(),
        [(1000, Some(50_000)), (50_000, Some(50_000))],
        "its first read, then nothing until its end"
    );
    assert!(DOWNLOAD_TOLD_EVERY >= Duration::from_millis(200));
    assert!(DOWNLOAD_TOLD_EVERY <= Duration::from_millis(500));
}

#[test]
fn a_cancelled_download_fails_its_next_read_in_the_cancels_words() {
    let (download, told) = told();
    let archive = Chunked {
        chunks: 50,
        size: 1000,
    };
    let reads = Progressing::new(&archive, &download, Duration::ZERO);
    let mut body = reads.fetch("https://engine.test/a.zip").unwrap();
    let mut buf = [0_u8; 4096];
    assert_eq!(body.read(&mut buf).unwrap(), 1000);

    download.cancel();

    let why = body.read(&mut buf).unwrap_err();
    assert_eq!(why.to_string(), purlis_host::ENGINE_DOWNLOAD_CANCELLED);
    assert_eq!(told.lock().unwrap().len(), 1, "nothing more is told");
}

#[test]
fn a_cancelled_download_keeps_nothing_in_the_engines_folder() {
    use purlis_browser_engine::install::{EngineSettings, Engines, Manifest, NoSigner, Platform};
    let folder = tempfile::tempdir().unwrap();
    let archive = vec![7_u8; 10_000];
    let digest: String = {
        use sha2::Digest as _;
        sha2::Sha256::digest(&archive)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    let url = "https://engine.test/155.0.8059.79/linux64/chrome-linux64.zip";
    let manifest = Manifest::parse(
        format!(
            r#"{{"version":"155.0.8059.79","issued":"2026-10-15T00:00:00Z","downloads":[{{"platform":"linux64","url":"{url}","sha256":"{digest}"}}]}}"#
        )
        .as_bytes(),
    )
    .unwrap();
    let engines = Engines::new(
        folder.path(),
        Platform::Linux64,
        manifest,
        Box::new(NoSigner),
    );
    // The person cancelled it as it began.
    let (download, _told) = told();
    download.cancel();
    let archive = Chunked {
        chunks: 10,
        size: 1000,
    };
    let reads = Progressing::new(&archive, &download, Duration::ZERO);

    let refused = engines
        .prepare(
            &EngineSettings::default(),
            &reads,
            chrono::DateTime::parse_from_rfc3339("2026-10-15T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        )
        .unwrap_err();

    assert!(
        download_failed_said(&refused).contains("cancelled"),
        "{}",
        download_failed_said(&refused)
    );
    let left: Vec<_> = std::fs::read_dir(folder.path())
        .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "nothing of it is kept: {left:?}");
}

#[test]
fn a_download_that_failed_says_what_to_do() {
    use purlis_browser_engine::install::Refused;
    let failed = Refused::Download(std::io::Error::other("connection reset"));
    let said = download_failed_said(&failed);
    assert!(said.contains("connection reset"), "{said}");
    assert!(said.contains("open the browser again"), "{said}");
    let digest = Refused::Digest {
        expected: "aa".into(),
        got: "bb".into(),
    };
    let said = download_failed_said(&digest);
    assert!(said.contains("was not kept"), "{said}");
    assert!(said.contains("open the browser again"), "{said}");
}

#[test]
fn why_devtools_closed_is_said_in_a_word_their_view_reads() {
    use purlis_browser_engine::drive::InspectEnded;
    for why in [
        InspectEnded::Closed,
        InspectEnded::PageGone,
        InspectEnded::FellBehind,
        InspectEnded::TooLarge,
        InspectEnded::Moved,
        InspectEnded::Ended,
    ] {
        assert!(
            purlis_session_protocol::view::frames::CLOSED_WHY.contains(&super::closed_word(why)),
            "{why:?}"
        );
    }
}

/// A project whose state folder holds a vault's file, and a file of its own beside it.
fn a_project_with_a_vault() -> (tempfile::TempDir, std::path::PathBuf) {
    let made = tempfile::tempdir().expect("a directory");
    let project = made.path().join("project");
    let vaults = project.join(crate::names::STATE_DIR.write).join("vaults");
    std::fs::create_dir_all(&vaults).expect("made");
    std::fs::write(vaults.join("forge.json"), "secret").expect("written");
    std::fs::write(project.join("notes.txt"), "notes").expect("written");
    let project = project.canonicalize().expect("there");
    (made, project)
}

#[test]
fn an_upload_the_host_makes_refuses_what_purlis_keeps_from_the_projects_chats() {
    // #2020: the host reads what purlis keeps from the project's chats itself, whatever the
    // window judged, and refuses it by what it is.
    let (made, project) = a_project_with_a_vault();
    let into = made.path().join("browser/uploads/0");
    let vault = format!("{}/vaults/forge.json", crate::names::STATE_DIR.write);
    let refused = super::staged_files(&project, &project, &[vault], &into).unwrap_err();
    assert!(refused.contains("keeps from every chat"), "{refused}");
    let copies =
        super::staged_files(&project, &project, &["notes.txt".to_owned()], &into).expect("read");
    assert_eq!(copies.len(), 1);
}

#[test]
fn a_save_the_host_makes_never_lands_in_what_purlis_keeps_from_the_projects_chats() {
    let (made, project) = a_project_with_a_vault();
    let download = made.path().join("GUID-1");
    std::fs::write(&download, "downloaded").expect("written");
    let to = format!("{}/vaults/planted.json", crate::names::STATE_DIR.write);
    let refused = super::saved_file(&project, &download, &project, &to).unwrap_err();
    assert!(refused.contains("keeps from every chat"), "{refused}");
    assert!(
        !project
            .join(crate::names::STATE_DIR.write)
            .join("vaults/planted.json")
            .exists()
    );
    assert_eq!(
        super::saved_file(&project, &download, &project, "saved.txt").expect("saved"),
        10
    );
}

/// #5167: a browser opened from a kept login whose form is on another site reaches that site from
/// its start, as one logged in afresh does once its sign-in is over: the agent is kept off its
/// pages, and every other site stays refused.
#[test]
fn a_browser_opened_from_a_kept_login_reaches_its_form_s_site_from_its_start() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    use crate::sandbox::browser_reach::{Lane, ReachProxy};

    let server = || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
        let port = listener.local_addr().expect("an address").port();
        (listener, port)
    };
    let (_page_site, page_port) = server();
    let (_form_site, form_port) = server();
    let (_other_site, other_port) = server();
    let reach = BrowserReach {
        listed: [page_port, form_port, other_port]
            .iter()
            .flat_map(|port| crate::sandbox::localport::reach_entries(*port))
            .map(|host| Listed {
                host,
                layer: Layer::Chat,
            })
            .collect(),
        ..BrowserReach::default()
    };
    let sites = std::sync::Arc::new(super::judge::SiteJudge::of(&reach).unwrap());
    let (listed, _) = reach_of(Some(&reach)).unwrap();
    let proxy =
        ReachProxy::bound_by(Lane::of(listed), sites.new_sites(), sites.bound().clone()).unwrap();
    let status = |port: u16| -> String {
        let mut stream = TcpStream::connect(proxy.address()).expect("the proxy");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream
            .write_all(format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n\r\n").as_bytes())
            .unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        line.trim_end().to_owned()
    };
    let form = Site::of("127.0.0.1", form_port);
    let at = LoginSites {
        page: Site::of("127.0.0.1", page_port),
        form: Some(form.clone()),
    };

    super::opened_from_kept(&proxy, &sites, &at).expect("held");

    assert_eq!(status(page_port), "HTTP/1.1 200 Connection established");
    assert_eq!(status(form_port), "HTTP/1.1 200 Connection established");
    assert_eq!(status(other_port), "HTTP/1.1 403 Forbidden");
    assert_eq!(
        proxy.bound().kept_off("127.0.0.1", form_port),
        Some(form),
        "no sign-in is open"
    );
    assert_eq!(proxy.bound().kept_off("127.0.0.1", page_port), None);
}

/// A done download whose site the browser signs in on, as the host judges bringing it in.
fn download_from(kept_off: Option<&str>, from_no_origin: bool) -> super::DownloadsSite {
    super::DownloadsSite {
        kept_off: kept_off.map(str::to_owned),
        from_no_origin,
        began_on_the_sign_ins_page: false,
        keeps_off: None,
        signing_on: None,
    }
}

#[test]
fn a_download_of_the_site_a_browser_signs_in_on_is_not_brought_in_by_the_chat() {
    let of_it = download_from(Some("auth.test:443"), false);
    let why = of_it.refused("d1").expect("refused");
    assert!(why.contains("came from auth.test:443"), "{why}");

    assert_eq!(
        download_from(None, false).refused("d1"),
        None,
        "one of another site is the chat's"
    );
}

#[test]
fn a_download_from_no_site_is_refused_while_the_browser_signs_in_on_one() {
    let outside = super::DownloadsSite {
        keeps_off: Some("auth.test:443".to_owned()),
        ..download_from(None, true)
    };
    let why = outside.refused("d1").expect("refused outside a sign-in");
    assert!(why.contains("only while browser_login signs in"), "{why}");

    let elsewhere = super::DownloadsSite {
        signing_on: Some("auth.test:443".to_owned()),
        ..download_from(None, true)
    };
    let why = elsewhere
        .refused("d1")
        .expect("refused during a sign-in, begun on another page");
    assert!(why.contains("signs in on auth.test:443 now"), "{why}");

    let on_its_page = super::DownloadsSite {
        began_on_the_sign_ins_page: true,
        ..elsewhere
    };
    assert_eq!(
        on_its_page.refused("d1"),
        None,
        "begun on the sign-in's own page"
    );
    assert_eq!(
        download_from(None, true).refused("d1"),
        None,
        "a browser that signs in nowhere"
    );
}

/// Where a stand-in browser says each download began, and which downloads it was asked of.
#[derive(Default)]
struct Began {
    kept_off: Option<&'static str>,
    on_the_sign_ins_page: bool,
    asked: std::sync::Mutex<Vec<String>>,
}

impl super::DownloadsWhence for Began {
    fn kept_off(&self, id: &str) -> Option<String> {
        self.asked.lock().unwrap().push(id.to_owned());
        self.kept_off.map(str::to_owned)
    }

    fn began_on_the_sign_ins_page(&self, id: &str) -> bool {
        self.asked.lock().unwrap().push(id.to_owned());
        self.on_the_sign_ins_page
    }
}

/// Download `id`, done, from `origin`.
fn done_download(id: &str, origin: Option<&str>) -> purlis_browser_engine::drive::DownloadInfo {
    purlis_browser_engine::drive::DownloadInfo {
        id: id.to_owned(),
        name: "report.csv".to_owned(),
        origin: origin.map(str::to_owned),
        received: 22,
        total: Some(22),
        state: purlis_browser_engine::drive::DownloadState::Done,
        file: "4f1c".to_owned(),
    }
}

#[test]
fn a_download_is_judged_on_what_its_browser_and_its_login_say_of_it_now() {
    use super::DownloadsSite;
    use crate::sandbox::browser_reach::{Bound, LoginSites, Site};
    let bound = Bound::default();
    let sign_in = bound
        .to(&LoginSites {
            page: Site::of("app.test", 443),
            form: Some(Site::of("auth.test", 443)),
        })
        .unwrap()
        .expect("a sign-in opens on the form's site");

    // During the sign-in: what the browser says of this download, and the site signed in on.
    let browser = Began {
        kept_off: Some("auth.test:443"),
        on_the_sign_ins_page: true,
        ..Began::default()
    };
    assert_eq!(
        DownloadsSite::of(
            &done_download("d2", Some("https://auth.test")),
            &browser,
            Some(&bound)
        ),
        DownloadsSite {
            kept_off: Some("auth.test:443".to_owned()),
            from_no_origin: false,
            began_on_the_sign_ins_page: true,
            keeps_off: None,
            signing_on: Some("auth.test:443".to_owned()),
        }
    );
    assert_eq!(
        *browser.asked.lock().unwrap(),
        ["d2", "d2"],
        "the browser is asked of this download"
    );

    // Once it is over: the site the browser signs in on keeps one from no origin.
    bound.sign_in_over(sign_in);
    let from_nowhere = done_download("d3", None);
    assert_eq!(
        DownloadsSite::of(&from_nowhere, &Began::default(), Some(&bound)),
        DownloadsSite {
            kept_off: None,
            from_no_origin: true,
            began_on_the_sign_ins_page: false,
            keeps_off: Some("auth.test:443".to_owned()),
            signing_on: None,
        }
    );

    // A browser given no reach holds no login.
    assert_eq!(
        DownloadsSite::of(&from_nowhere, &Began::default(), None),
        DownloadsSite {
            kept_off: None,
            from_no_origin: true,
            began_on_the_sign_ins_page: false,
            keeps_off: None,
            signing_on: None,
        }
    );
}
