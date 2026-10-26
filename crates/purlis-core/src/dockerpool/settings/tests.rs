// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use serde_json::json;

/// Docker settings holding every kind of setting, those purlis hands on and those it does not.
fn written() -> serde_json::Value {
    json!({
        "auths": {"registry.example": {"auth": "dXNlcjpwYXNz"}},
        "credsStore": "desktop",
        "credHelpers": {
            "gcr.io": "gcloud",
            "planted.example": "planted",
            "path.example": "../../tmp/x"
        },
        "cliPluginsExtraDirs": ["/tmp/plugins"],
        "plugins": {"scout": {"hooks": "pull,run"}},
        "features": {"hooks": "true"},
        "currentContext": "elsewhere",
        "proxies": {"default": {"httpProxy": "http://proxy.example:3128"}},
        "credsStoreUnknown": "x"
    })
}

/// Docker's own settings are handed on as their `auths` and the helpers Docker's own folders
/// hold, and nothing else (#2752, the person's ruling of 2026-10-20).
#[test]
fn only_auths_and_the_helpers_dockers_own_folders_hold_are_handed_on() {
    let held = |name: &str| matches!(name, "desktop" | "gcloud");
    assert_eq!(
        checked(&written(), &held),
        json!({
            "auths": {"registry.example": {"auth": "dXNlcjpwYXNz"}},
            "credsStore": "desktop",
            "credHelpers": {"gcr.io": "gcloud"}
        })
    );
    // A helper not in Docker's own folders is not named at all.
    assert_eq!(
        checked(&written(), &|_| false),
        json!({"auths": {"registry.example": {"auth": "dXNlcjpwYXNz"}}})
    );
    // A name that could be a path is never asked about.
    let asked = std::cell::RefCell::new(Vec::new());
    let _ = checked(&written(), &|name| {
        asked.borrow_mut().push(name.to_owned());
        true
    });
    assert!(
        !asked.borrow().iter().any(|name| name.contains('/')),
        "{asked:?}"
    );
    // Settings of another shape hand on nothing.
    assert_eq!(checked(&json!([1, 2]), &held), json!({}));
    assert_eq!(checked(&json!({"auths": "x"}), &held), json!({}));
}

#[test]
fn a_helper_is_held_only_where_dockers_own_folders_have_it() {
    let (own, home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    stand_in::program(own.path(), "docker-credential-desktop", "#!/bin/sh\n");
    stand_in::program(home.path(), "docker-credential-planted", "#!/bin/sh\n");
    let folders = vec![own.path().to_path_buf()];
    assert!(held_in(&folders, "desktop"));
    assert!(!held_in(&folders, "planted"));
    assert!(!held_in(&folders, "../desktop"));
    // Docker's own folders are where its installers put `docker`, and no folder of the home.
    let folders = docker_folders();
    assert!(
        folders.contains(&PathBuf::from("/usr/local/bin")),
        "{folders:?}"
    );
    assert!(
        folders.iter().all(|folder| !folder.starts_with("/Users")),
        "{folders:?}"
    );
}

/// A stand-in for an engine's socket at `at`, its folders made.
fn socket_at(at: &Path) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, "").unwrap();
}

/// The engine is the window's own `DOCKER_HOST` where it has one, else the system's socket,
/// else the socket of the first installer in the home that has one, in its own folder: never a
/// context. Each only where, as it resolves, no chat can put another socket in its place
/// (#2752).
#[test]
fn the_engine_is_pinned_to_a_socket_no_chat_can_replace_and_never_a_context() {
    let (home_dir, system_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let home = home_dir.path().canonicalize().unwrap();
    let system = system_dir.path().join("docker.sock");
    let places = |system: &Path| Places {
        home: Some(&home),
        tmp: None,
        runtime: None,
        system: Box::leak(system.to_path_buf().into_boxed_path()),
    };
    // No socket anywhere: none.
    assert_eq!(engine(None, &places(&system)), Ok(None));
    // Docker Desktop's, in the home: its own folder, which chats may not write.
    socket_at(&home.join(".docker/run/docker.sock"));
    assert_eq!(
        engine(None, &places(&system)),
        Ok(Some(format!(
            "unix://{}",
            home.join(".docker/run/docker.sock").display()
        )))
    );
    // The system's socket, linked to Colima's in the home: taken, it is held.
    socket_at(&home.join(".colima/default/docker.sock"));
    std::os::unix::fs::symlink(home.join(".colima/default/docker.sock"), &system).unwrap();
    assert_eq!(
        engine(None, &places(&system)),
        Ok(Some(format!("unix://{}", system.display())))
    );
    // The system's socket, linked to a folder of the home a chat may write: refused, naming it.
    let linked = system_dir.path().join("elsewhere.sock");
    socket_at(&home.join("projects/docker.sock"));
    std::os::unix::fs::symlink(home.join("projects/docker.sock"), &linked).unwrap();
    let refused = engine(None, &places(&linked)).unwrap_err();
    assert!(
        refused
            .0
            .contains(&home.join("projects").display().to_string()),
        "{refused}"
    );
    assert!(refused.0.contains("docker context"), "{refused}");
    // The window's own DOCKER_HOST is held to the same.
    let window = format!("unix://{}", home.join("projects/docker.sock").display());
    assert!(engine(Some(&window), &places(&system)).is_err());
    let lima = home.join(".lima/default/sock/docker.sock");
    socket_at(&lima);
    let window = format!("unix://{}", lima.display());
    assert_eq!(engine(Some(&window), &places(&system)), Ok(None));
    assert_eq!(
        engine(Some("tcp://127.0.0.1:2375"), &places(&system)),
        Ok(None)
    );
    // An installer's folder that is a link in its place holds nothing.
    let (other_dir, aside) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let other = other_dir.path().canonicalize().unwrap();
    socket_at(&aside.path().join("run/docker.sock"));
    std::os::unix::fs::symlink(aside.path(), other.join(".docker")).unwrap();
    let nowhere = system_dir.path().join("none.sock");
    let other_places = Places {
        home: Some(&other),
        tmp: None,
        runtime: None,
        system: &nowhere,
    };
    assert_eq!(engine(None, &other_places), Ok(None));
    let window = format!("unix://{}", other.join(".docker/run/docker.sock").display());
    assert!(engine(Some(&window), &other_places).is_err());
}

/// **A pool's engine is this machine's only on a local socket** (#2050): a browser login is
/// filled only on such a pool's runner, so a network engine (`tcp://`, `ssh://`) is refused in
/// words, as is no engine.
#[test]
fn an_engine_is_on_this_machine_only_where_it_is_a_trusted_local_socket() {
    let (home_dir, system_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let home = home_dir.path().canonicalize().unwrap();
    let system = system_dir.path().join("docker.sock");
    let places = Places {
        home: Some(&home),
        tmp: None,
        runtime: None,
        system: &system,
    };
    assert!(
        engine_on_this_machine(None, &places)
            .unwrap_err()
            .contains("no Docker engine")
    );
    for remote in [
        "tcp://127.0.0.1:2375",
        "tcp://build-box.example:2376",
        "ssh://me@build-box.example",
        "npipe:////./pipe/docker_engine",
    ] {
        let refused = engine_on_this_machine(Some(remote), &places).unwrap_err();
        assert!(refused.contains(remote), "{refused}");
        assert!(
            refused.contains("cannot tell is on this machine"),
            "{refused}"
        );
    }
    let desktop = home.join(".docker/run/docker.sock");
    socket_at(&desktop);
    assert_eq!(engine_on_this_machine(None, &places), Ok(desktop.clone()));
    let window = format!("unix://{}", desktop.display());
    assert_eq!(engine_on_this_machine(Some(&window), &places), Ok(desktop));
    socket_at(&home.join("projects/docker.sock"));
    let writable = format!("unix://{}", home.join("projects/docker.sock").display());
    assert!(engine_on_this_machine(Some(&writable), &places).is_err());
}

#[test]
fn only_a_folder_root_alone_writes_is_root_s() {
    assert!(only_root_writes(Path::new("/")));
    assert!(!only_root_writes(tempfile::tempdir().unwrap().path()));
}

/// A run's settings are made fresh in a folder of its own, this user's alone, from where the
/// window keeps Docker's settings, and gone once the run is.
#[test]
fn each_run_gets_a_checked_copy_of_its_own_that_goes_with_it() {
    use std::os::unix::fs::PermissionsExt;
    let (home, data, elsewhere) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    std::fs::create_dir_all(home.path().join(".docker")).unwrap();
    std::fs::write(
        home.path().join(".docker/config.json"),
        written().to_string(),
    )
    .unwrap();
    std::fs::write(
        elsewhere.path().join("config.json"),
        json!({"auths": {"other.example": {}}, "currentContext": "x"}).to_string(),
    )
    .unwrap();
    let settings_in = data.path().join(DIR);
    let source = |docker_config: Option<&'static Path>| Source {
        settings_in: &settings_in,
        path: OsString::from("/usr/bin"),
        home: Some(home.path()),
        docker_config,
        docker_host: Some("tcp://127.0.0.1:2375"),
        tmp: None,
        runtime: None,
        system: Path::new("/nonexistent/docker.sock"),
    };
    let prepared = prepare(&source(None)).unwrap();
    let named = |prepared: &Prepared, name: &str| {
        prepared
            .env
            .iter()
            .find(|(one, _)| *one == name)
            .map(|(_, value)| PathBuf::from(value))
    };
    let folder = named(&prepared, "DOCKER_CONFIG").unwrap();
    assert!(folder.starts_with(&settings_in));
    let copy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(folder.join("config.json")).unwrap())
            .unwrap();
    assert_eq!(copy["auths"], written()["auths"]);
    for gone in [
        "cliPluginsExtraDirs",
        "plugins",
        "currentContext",
        "proxies",
        "features",
    ] {
        assert!(copy.get(gone).is_none(), "{gone}: {copy}");
    }
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&folder.join("config.json")), 0o600);
    assert_eq!(mode(&folder), 0o700);
    assert_eq!(mode(&settings_in), 0o700);
    // The window's own DOCKER_HOST is the run's, inherited.
    assert_eq!(named(&prepared, "DOCKER_HOST"), None);
    assert_eq!(named(&prepared, "PATH"), Some(PathBuf::from("/usr/bin")));
    drop(prepared);
    assert!(!folder.exists());

    // The window's DOCKER_CONFIG names where Docker's settings are.
    let leaked: &'static Path = Box::leak(elsewhere.path().to_path_buf().into_boxed_path());
    let prepared = prepare(&source(Some(leaked))).unwrap();
    let folder = named(&prepared, "DOCKER_CONFIG").unwrap();
    let copy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(folder.join("config.json")).unwrap())
            .unwrap();
    assert_eq!(copy, json!({"auths": {"other.example": {}}}));
}

/// Settings that are not there, not a file, or not JSON are taken as none.
#[test]
fn settings_that_cannot_be_read_are_taken_as_none() {
    let (home, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let settings_in = data.path().join(DIR);
    let copy_of = || {
        let prepared = prepare(&Source {
            settings_in: &settings_in,
            path: OsString::new(),
            home: Some(home.path()),
            docker_config: None,
            docker_host: None,
            tmp: None,
            runtime: None,
            system: Path::new("/nonexistent/docker.sock"),
        })
        .unwrap();
        let folder = PathBuf::from(
            &prepared
                .env
                .iter()
                .find(|(name, _)| *name == "DOCKER_CONFIG")
                .unwrap()
                .1,
        );
        std::fs::read_to_string(folder.join("config.json")).unwrap()
    };
    assert_eq!(copy_of(), "{}");
    std::fs::create_dir_all(home.path().join(".docker/config.json")).unwrap();
    assert_eq!(copy_of(), "{}");
    std::fs::remove_dir(home.path().join(".docker/config.json")).unwrap();
    std::fs::write(home.path().join(".docker/config.json"), "{not json").unwrap();
    assert_eq!(copy_of(), "{}");
}

/// `folder`'s time of last change, set back to `ago` before now.
fn aged(folder: &Path, ago: std::time::Duration) {
    let then = std::time::SystemTime::now() - ago;
    std::fs::File::open(folder)
        .unwrap()
        .set_modified(then)
        .unwrap();
}

/// A copy a run that ended outright left behind is swept once it is older than a day, as a
/// run is prepared and as the app starts (#2893); a run still going keeps its own, however
/// old, and nothing but a run's folder is touched.
#[test]
fn copies_older_than_a_day_are_swept_and_a_running_runs_never_is() {
    let (home, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let settings_in = data.path().join(DIR);
    let source = || Source {
        settings_in: &settings_in,
        path: OsString::new(),
        home: Some(home.path()),
        docker_config: None,
        docker_host: None,
        tmp: None,
        runtime: None,
        system: Path::new("/nonexistent/docker.sock"),
    };
    let day = std::time::Duration::from_secs(24 * 60 * 60);
    // A run still going, whose copy is two days old.
    let running = prepare(&source()).unwrap();
    let running_folder = PathBuf::from(
        &running
            .env
            .iter()
            .find(|(name, _)| *name == "DOCKER_CONFIG")
            .unwrap()
            .1,
    );
    aged(&running_folder, 2 * day);
    // Two left behind: one two days old, one an hour old; and a folder that is no run's.
    let left_old = settings_in.join("run-left-old");
    let left_young = settings_in.join("run-left-young");
    let other = settings_in.join("mine");
    for folder in [&left_old, &left_young, &other] {
        std::fs::create_dir_all(folder).unwrap();
        std::fs::write(folder.join("config.json"), "{}").unwrap();
    }
    aged(&left_old, 2 * day);
    aged(&left_young, std::time::Duration::from_secs(60 * 60));
    aged(&other, 2 * day);

    // As a run is prepared.
    let next = prepare(&source()).unwrap();
    assert!(!left_old.exists());
    assert!(left_young.exists());
    assert!(other.exists());
    assert!(running_folder.exists(), "a running run's copy was swept");
    drop(next);

    // As the app starts.
    let left_again = settings_in.join("run-left-again");
    std::fs::create_dir_all(&left_again).unwrap();
    aged(&left_again, 2 * day);
    assert_eq!(sweep(&settings_in, std::time::SystemTime::now()), 1);
    assert!(!left_again.exists());
    assert!(running_folder.exists());
    drop(running);
    assert!(!running_folder.exists());
}

/// Nothing is swept where the folder is not there, or is a link.
#[test]
fn a_settings_folder_that_is_not_one_is_never_swept() {
    let (data, elsewhere) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    assert_eq!(
        sweep(&data.path().join(DIR), std::time::SystemTime::now()),
        0
    );
    let old = elsewhere.path().join("run-old");
    std::fs::create_dir_all(&old).unwrap();
    aged(&old, std::time::Duration::from_secs(3 * 24 * 60 * 60));
    std::os::unix::fs::symlink(elsewhere.path(), data.path().join(DIR)).unwrap();
    assert_eq!(
        sweep(&data.path().join(DIR), std::time::SystemTime::now()),
        0
    );
    assert!(old.exists());
}
