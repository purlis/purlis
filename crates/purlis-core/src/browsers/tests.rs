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

// ---- A chat's acts, its answers and its reach (#2013) ----

use std::time::Duration;

use purlis_browser_engine::drive::{
    self, Answer, Call, Content, DragTo, Field, PageId, PageInfo, Ref, Settled,
};
use purlis_host::protocol::browsing::{self as wire, Act, Browsed, BrowserReach, PageSaid};
use purlis_host::protocol::session::{Layer, Listed};

use super::{aim_of, browsed, call_of, input_of, reach_of};
use crate::sandbox::browser_reach::Site;

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
    })
    .unwrap();
    assert_eq!(login.origin, "https://shop.example.test");
    assert_eq!(site, Site::of("shop.example.test", 443));
    assert!(login.user.is_none());
    let local = super::login_of(&LogIn {
        address: "http://127.0.0.1:8123/login".into(),
        user: Some(Secret::new("fixture-user".into())),
        password: Secret::new("fixture-word".into()),
        keep: None,
    })
    .unwrap();
    assert_eq!(local.0.origin, "http://127.0.0.1:8123");
    assert_eq!(local.1, Site::of("127.0.0.1", 8123));
    let refused = super::login_of(&LogIn {
        address: "file:///etc/passwd".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
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
    })
    .unwrap_err();
    assert!(refused.contains("plain http"), "{refused}");
    let (login, site) = super::login_of(&LogIn {
        address: "https://bücher.example/login".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep: None,
    })
    .unwrap();
    assert_eq!(login.origin, "https://xn--bcher-kva.example");
    assert_eq!(site, Site::of("xn--bcher-kva.example", 443));
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
    assert_eq!(status(other_port), "HTTP/1.1 200 Connection established");
    let login_site = Site::of("127.0.0.1", login_port);
    let bound = super::performed(&sites, &proxy, &login_site, "http://127.0.0.1", |bind| {
        bind()
    })
    .expect("performed");
    assert_eq!(bound, Ok(()));
    assert_eq!(status(other_port), "HTTP/1.1 403 Forbidden");
    assert_eq!(status(login_port), "HTTP/1.1 200 Connection established");
    let ran = std::cell::Cell::new(false);
    let refused = super::performed(
        &sites,
        &proxy,
        &Site::of("127.0.0.1", other_port),
        "http://127.0.0.1",
        |_| ran.set(true),
    )
    .unwrap_err();
    assert!(refused.contains("holds a login at 127.0.0.1:"), "{refused}");
    assert!(!ran.get(), "a second login at another site runs nothing");
}

// ---- Kept logins (#2021) ----

fn kept_login() -> wire::KeptLogin {
    wire::KeptLogin {
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
