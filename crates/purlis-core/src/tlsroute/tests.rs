// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use super::*;

// ---------------------------------------------------------------------------------------------
// The route.

#[test]
fn a_tcp_route_without_a_port_is_dockers_tls_port() {
    let route = Route::parse("tcp://build-box").unwrap();
    assert_eq!(route.host(), "build-box");
    assert_eq!(route.port(), DOCKER_TLS_PORT);
    assert_eq!(route.spelled(), "tcp://build-box:2376");
    assert_eq!(Route::parse("tcp://build-box:2377/").unwrap().port(), 2377);
    assert_eq!(
        Route::parse("tcp://[::1]:2376").unwrap().spelled(),
        "tcp://[::1]:2376"
    );
}

#[test]
fn a_tcp_route_is_a_host_and_a_port_and_nothing_else() {
    for refused in [
        "tcp://me@build-box:2376",
        "tcp://build-box:2376/var/run/docker.sock",
        "tcp://-oProxyCommand=x",
        "tcp://build-box:0",
        "tcp://",
        "ssh://build-box",
        "build-box:2376",
    ] {
        assert!(Route::parse(refused).is_err(), "{refused} was taken");
    }
}

#[test]
fn plain_tcp_without_tls_files_is_refused_with_adr_0123s_sentence() {
    assert_eq!(
        route_with("tcp://build-box:2375", None).unwrap_err(),
        crate::sshroute::PLAIN_TCP
    );
    assert!(crate::sshroute::PLAIN_TCP.contains("anyone who can reach that port"));
}

// ---------------------------------------------------------------------------------------------
// The fingerprint.

#[test]
fn a_fingerprint_is_kept_in_lower_hex_and_shown_as_openssl_prints_it() {
    let fingerprint = Fingerprint::of_der(b"a certificate");
    let kept = fingerprint.spelled();
    assert!(kept.starts_with("sha256:"));
    assert_eq!(kept.len(), "sha256:".len() + 64);
    assert_eq!(Fingerprint::parse(&kept).unwrap(), fingerprint);
    let shown = fingerprint.shown();
    assert_eq!(shown.split(':').count(), 32);
    assert_eq!(Fingerprint::parse(&shown).unwrap(), fingerprint);
    assert_eq!(
        Fingerprint::parse(&format!("sha256 Fingerprint={shown}")).unwrap(),
        fingerprint
    );
    assert!(Fingerprint::parse("sha256:abc").is_err());
    assert!(Fingerprint::parse(&"zz".repeat(32)).is_err());
}

// ---------------------------------------------------------------------------------------------
// Certificates made for a test, with the system's openssl, as dind makes its own.

struct Pki {
    dir: tempfile::TempDir,
}

impl Pki {
    fn new() -> Pki {
        let pki = Pki {
            dir: tempfile::tempdir().unwrap(),
        };
        pki.ca("ca");
        pki.ca("other-ca");
        pki.signed("server", "ca", "localhost", "DNS:localhost", "serverAuth");
        pki.signed("client", "ca", "client", "DNS:client", "clientAuth");
        pki.signed("stranger", "other-ca", "client", "DNS:client", "clientAuth");
        pki.signed(
            "server-of-other",
            "other-ca",
            "localhost",
            "DNS:localhost",
            "serverAuth",
        );
        pki
    }

    fn at(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn openssl(&self, args: &[&str]) {
        let ran = crate::forklock::output(
            std::process::Command::new("openssl")
                .args(args)
                .current_dir(self.dir.path()),
        )
        .expect("the system's openssl");
        assert!(
            ran.status.success(),
            "openssl {args:?}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }

    fn key(&self, name: &str) {
        self.openssl(&[
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-pkeyopt",
            "ec_param_enc:named_curve",
            "-out",
            &format!("{name}-key.pem"),
        ]);
    }

    fn ca(&self, name: &str) {
        self.key(name);
        std::fs::write(
            self.at(&format!("{name}.ext")),
            "basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n",
        )
        .unwrap();
        self.openssl(&[
            "req",
            "-new",
            "-key",
            &format!("{name}-key.pem"),
            "-subj",
            &format!("/CN={name}"),
            "-out",
            &format!("{name}.csr"),
        ]);
        self.openssl(&[
            "x509",
            "-req",
            "-sha256",
            "-in",
            &format!("{name}.csr"),
            "-signkey",
            &format!("{name}-key.pem"),
            "-days",
            "3650",
            "-extfile",
            &format!("{name}.ext"),
            "-out",
            &format!("{name}.pem"),
        ]);
    }

    fn signed(&self, name: &str, by: &str, cn: &str, san: &str, usage: &str) {
        self.key(name);
        std::fs::write(
            self.at(&format!("{name}.ext")),
            format!(
                "basicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature\n\
                 extendedKeyUsage={usage}\nsubjectAltName={san}\n"
            ),
        )
        .unwrap();
        self.openssl(&[
            "req",
            "-new",
            "-key",
            &format!("{name}-key.pem"),
            "-subj",
            &format!("/CN={cn}"),
            "-out",
            &format!("{name}.csr"),
        ]);
        self.openssl(&[
            "x509",
            "-req",
            "-sha256",
            "-in",
            &format!("{name}.csr"),
            "-CA",
            &format!("{by}.pem"),
            "-CAkey",
            &format!("{by}-key.pem"),
            "-CAcreateserial",
            "-days",
            "365",
            "-extfile",
            &format!("{name}.ext"),
            "-out",
            &format!("{name}.pem"),
        ]);
    }

    fn files(&self, client: &str) -> Files {
        Files::new(
            self.at("ca.pem"),
            self.at(&format!("{client}.pem")),
            KeyFrom::File(self.at(&format!("{client}-key.pem"))),
        )
        .unwrap()
    }

    fn fingerprint(&self, ca: &str) -> Fingerprint {
        read_ca(&self.at(&format!("{ca}.pem")), "box")
            .unwrap()
            .fingerprint
    }
}

// ---------------------------------------------------------------------------------------------
// The files, checked before every run.

#[test]
fn a_ca_file_that_holds_the_pinned_certificate_is_handed_to_the_run_as_it_was_read() {
    let pki = Pki::new();
    let through = Through {
        route: Route::parse("tcp://box").unwrap(),
        files: pki.files("client"),
        ca: pki.fingerprint("ca"),
        engine: None,
    };
    let material = through.material().unwrap();
    assert_eq!(material.ca, std::fs::read(pki.at("ca.pem")).unwrap());
    assert_eq!(material.cert, std::fs::read(pki.at("client.pem")).unwrap());
    assert_eq!(
        *material.key,
        std::fs::read(pki.at("client-key.pem")).unwrap()
    );
}

#[test]
fn a_ca_file_that_no_longer_matches_the_pin_refuses_the_run_naming_the_file() {
    let pki = Pki::new();
    let pinned = pki.fingerprint("ca");
    // The file is replaced by another CA's under the same name.
    std::fs::copy(pki.at("other-ca.pem"), pki.at("ca.pem")).unwrap();
    let through = Through {
        route: Route::parse("tcp://box").unwrap(),
        files: pki.files("client"),
        ca: pinned,
        engine: None,
    };
    match through.material() {
        Err(Finding::CaChanged {
            host,
            pinned: was,
            now,
            file,
        }) => {
            assert_eq!(host, "box");
            assert_eq!(was, pinned.shown());
            assert_eq!(now, pki.fingerprint("other-ca").shown());
            assert_eq!(file, pki.at("ca.pem").display().to_string());
        }
        Err(other) => panic!("refused otherwise: {other:?}"),
        Ok(_) => panic!("a CA other than the pinned one was handed to a run"),
    }
}

#[test]
fn a_ca_file_holding_two_certificates_or_none_is_refused_naming_it() {
    let pki = Pki::new();
    let both = [
        std::fs::read(pki.at("ca.pem")).unwrap(),
        std::fs::read(pki.at("other-ca.pem")).unwrap(),
    ]
    .concat();
    std::fs::write(pki.at("both.pem"), both).unwrap();
    std::fs::write(pki.at("none.pem"), "no certificate").unwrap();
    for name in ["both.pem", "none.pem", "missing.pem"] {
        match read_ca(&pki.at(name), "box") {
            Err(Finding::TlsFile { file, .. }) => {
                assert_eq!(file, pki.at(name).display().to_string());
            }
            other => panic!("{name}: {other:?}"),
        }
    }
}

#[test]
fn a_key_file_that_holds_no_private_key_is_refused_naming_it() {
    let pki = Pki::new();
    let files = Files::new(
        pki.at("ca.pem"),
        pki.at("client.pem"),
        KeyFrom::File(pki.at("client.pem")),
    )
    .unwrap();
    let through = Through {
        route: Route::parse("tcp://box").unwrap(),
        files,
        ca: pki.fingerprint("ca"),
        engine: None,
    };
    match through.material() {
        Err(Finding::TlsFile { file, why, .. }) => {
            assert_eq!(file, pki.at("client.pem").display().to_string());
            assert!(why.contains("private key"), "{why}");
        }
        other => panic!("{:?}", other.err()),
    }
}

#[test]
fn the_files_are_named_by_absolute_paths_and_a_vault_entry_by_plain_names() {
    assert!(
        Files::new(
            "ca.pem".into(),
            "/c.pem".into(),
            KeyFrom::File("/k.pem".into())
        )
        .is_err()
    );
    assert!(
        Files::new(
            "/ca.pem".into(),
            "/c.pem".into(),
            KeyFrom::File("k.pem".into())
        )
        .is_err()
    );
    let vault = |vault: &str, key: &str| {
        Files::new(
            "/ca.pem".into(),
            "/c.pem".into(),
            KeyFrom::Vault {
                plane: "/plane".into(),
                vault: vault.to_owned(),
                key: key.to_owned(),
            },
        )
    };
    assert!(vault("docker-box", "TLS_KEY").is_ok());
    assert!(vault("../x", "TLS_KEY").is_err());
    assert!(vault("docker-box", "--reveal").is_err());
}

#[test]
fn a_cas_subject_is_read_to_show_beside_its_fingerprint() {
    let pki = Pki::new();
    let ca = read_ca(&pki.at("ca.pem"), "box").unwrap();
    assert_eq!(ca.subject().as_deref(), Some("CN=ca"));
}

// ---------------------------------------------------------------------------------------------
// The doctor's handshake, against a server of the test's own.

/// Serves `tcp` with `server`'s certificate, asking for a client certificate signed by `ca`, and
/// answers Docker's ping where the handshake holds.
fn serve(pki: &Pki, server: &str, ca: &str, mut tcp: std::os::unix::net::UnixStream) {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_file(pki.at(&format!("{ca}.pem"))).unwrap())
        .unwrap();
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        provider.clone(),
    )
    .build()
    .unwrap();
    let chain = vec![CertificateDer::from_pem_file(pki.at(&format!("{server}.pem"))).unwrap()];
    let key = PrivateKeyDer::from_pem_file(pki.at(&format!("{server}-key.pem"))).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain, key)
        .unwrap();
    std::thread::spawn(move || {
        let mut tls = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        let mut stream = rustls::Stream::new(&mut tls, &mut tcp);
        let mut asked = Vec::new();
        let mut byte = [0u8; 1];
        while !asked.ends_with(b"\r\n\r\n") {
            match stream.read(&mut byte) {
                Ok(1) => asked.push(byte[0]),
                _ => return,
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK");
        stream.conn.send_close_notify();
        let _ = stream.flush();
    });
}

/// The doctor's handshake as `host` with `client`'s certificate, against [`serve`].
fn shake(pki: &Pki, host: &str, server: &str, client: &str) -> Result<(), Finding> {
    let route = Route::parse(&format!("tcp://{host}")).unwrap();
    let files = pki.files(client);
    let through = Through {
        route: route.clone(),
        files: files.clone(),
        ca: pki.fingerprint("ca"),
        engine: None,
    };
    let material = through.material().unwrap();
    let (mut ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
    serve(pki, server, "ca", theirs);
    let config = client_config(route.host(), &files, &material)?;
    handshake_over(&mut ours, &route, &files, config)
}

#[test]
fn an_engine_that_verifies_and_accepts_the_client_answers_its_ping() {
    let pki = Pki::new();
    shake(&pki, "localhost", "server", "client").unwrap();
}

#[test]
fn a_server_certificate_of_another_ca_is_said_to_be_not_signed_by_the_ca_named() {
    let pki = Pki::new();
    match shake(&pki, "localhost", "server-of-other", "client") {
        Err(Finding::NotSignedByCa { host, ca }) => {
            assert_eq!(host, "localhost");
            assert_eq!(ca, pki.at("ca.pem").display().to_string());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_server_certificate_for_another_name_is_said_with_the_names_it_is_for() {
    let pki = Pki::new();
    match shake(&pki, "127.0.0.1", "server", "client") {
        Err(Finding::WrongName { host, name }) => {
            assert_eq!(host, "127.0.0.1");
            assert!(name.contains("localhost"), "{name}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_client_certificate_the_engine_refuses_is_said_naming_the_certificate() {
    let pki = Pki::new();
    match shake(&pki, "localhost", "server", "stranger") {
        Err(Finding::ClientRefused { cert, .. }) => {
            assert_eq!(cert, pki.at("stranger.pem").display().to_string());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_expired_certificate_on_either_side_is_told_apart_by_rustlss_own_error() {
    use rustls::{AlertDescription, CertificateError, Error};
    assert_eq!(
        finding_of_tls(
            &Error::InvalidCertificate(CertificateError::Expired),
            "box",
            "/ca.pem",
            "/cert.pem"
        ),
        Finding::ServerExpired {
            host: "box".to_owned()
        }
    );
    assert_eq!(
        finding_of_tls(
            &Error::AlertReceived(AlertDescription::CertificateExpired),
            "box",
            "/ca.pem",
            "/cert.pem"
        ),
        Finding::ClientExpired {
            host: "box".to_owned(),
            cert: "/cert.pem".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// A run's copies.

/// A plane of the test's own with a plain-file vault `docker-box` holding `TLS_KEY`.
fn plane_with_the_key(key: &str) -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    let ctx = crate::secrets::Ctx::new(plane.path(), crate::secrets::Env::of(&[]));
    crate::secrets::registry::add_vault(
        &ctx,
        "docker-box",
        "plain-file",
        serde_json::json!({"file": "docker-box.json"})
            .as_object()
            .cloned()
            .unwrap(),
        None,
        false,
        false,
    )
    .unwrap();
    std::fs::write(
        plane.path().join("docker-box.json"),
        serde_json::json!({ "TLS_KEY": key }).to_string(),
    )
    .unwrap();
    plane
}

#[test]
fn a_key_kept_in_a_vault_is_on_disk_for_its_run_alone_even_when_the_run_fails() {
    let pki = Pki::new();
    let key = std::fs::read_to_string(pki.at("client-key.pem")).unwrap();
    let plane = plane_with_the_key(&key);
    // Read as the window reads it, but from the test's own plane, then kept in memory.
    forget_vault_keys();
    let read = vault_key_in(
        plane.path(),
        "docker-box",
        "TLS_KEY",
        crate::secrets::Env::of(&[]),
    )
    .unwrap();
    assert_eq!(*read, key.as_bytes());
    let files = Files::new(
        pki.at("ca.pem"),
        pki.at("client.pem"),
        KeyFrom::Vault {
            plane: plane.path().to_path_buf(),
            vault: "docker-box".to_owned(),
            key: "TLS_KEY".to_owned(),
        },
    )
    .unwrap();
    let through = Through {
        route: Route::parse("tcp://box").unwrap(),
        files,
        ca: pki.fingerprint("ca"),
        engine: None,
    };
    // A docker that notes what it was handed, then fails.
    let bin = tempfile::tempdir().unwrap();
    let seen = bin.path().join("seen");
    let docker = stand_in::program(
        bin.path(),
        "docker",
        &format!(
            "#!/bin/sh\n{{ echo \"$DOCKER_HOST $DOCKER_TLS_VERIFY\"; echo \"$DOCKER_CERT_PATH\"; \
             cat \"$DOCKER_CERT_PATH/key.pem\"; }} > '{}'\necho refused >&2\nexit 1\n",
            seen.display()
        ),
    );
    let settings = tempfile::tempdir().unwrap();
    let docker = crate::dockerpool::Docker::at(docker)
        .with_settings_in(settings.path().join("docker-settings"))
        .over_tls(through);
    assert!(docker.engine_id_said().is_err(), "the run fails");
    let seen = std::fs::read_to_string(seen).unwrap();
    let mut lines = seen.lines();
    assert_eq!(lines.next(), Some("tcp://box:2376 1"));
    let folder = PathBuf::from(lines.next().unwrap());
    assert!(folder.starts_with(settings.path()));
    assert!(seen.contains("PRIVATE KEY"), "the run was handed the key");
    assert!(
        !folder.exists(),
        "the run's folder, key and all, is gone after it failed"
    );
    let left: Vec<_> = walk(settings.path());
    assert!(
        left.iter().all(|path| !path.ends_with(KEY_PEM)),
        "no key is left anywhere: {left:?}"
    );
    forget_vault_keys();
}

#[test]
fn a_ca_that_isnt_the_pinned_one_makes_no_run_folder_and_runs_nothing() {
    let pki = Pki::new();
    let pinned = pki.fingerprint("other-ca");
    let through = Through {
        route: Route::parse("tcp://box").unwrap(),
        files: pki.files("client"),
        ca: pinned,
        engine: None,
    };
    let bin = tempfile::tempdir().unwrap();
    let ran = bin.path().join("ran");
    let docker = stand_in::program(
        bin.path(),
        "docker",
        &format!("#!/bin/sh\ntouch '{}'\n", ran.display()),
    );
    let settings = tempfile::tempdir().unwrap();
    let docker = crate::dockerpool::Docker::at(docker)
        .with_settings_in(settings.path().join("docker-settings"))
        .over_tls(through);
    let refused = docker.engine_id_said().unwrap_err();
    assert!(
        refused.0.contains("no longer holds the CA you trusted"),
        "{}",
        refused.0
    );
    assert!(
        !ran.exists(),
        "docker was run with a CA other than the pinned one"
    );
    assert!(
        walk(settings.path())
            .iter()
            .all(|path| !path.ends_with(KEY_PEM))
    );
}

#[test]
fn a_run_folder_left_holding_a_key_is_swept_at_once_and_one_still_running_is_not() {
    let settings = tempfile::tempdir().unwrap();
    let settings_in = settings.path().join("docker-settings");
    let going = crate::dockerpool::settings::run_folder(&settings_in).unwrap();
    going.keep(KEY_PEM, b"key").unwrap();
    let left = settings_in.join("run-left");
    std::fs::create_dir(&left).unwrap();
    std::fs::write(left.join(KEY_PEM), "key").unwrap();
    let without = settings_in.join("run-young");
    std::fs::create_dir(&without).unwrap();
    std::fs::write(without.join("config.json"), "{}").unwrap();
    let swept = crate::dockerpool::settings::sweep(&settings_in, std::time::SystemTime::now());
    assert_eq!(swept, 1);
    assert!(!left.exists(), "a key a run left behind went at once");
    assert!(
        going.path().join(KEY_PEM).exists(),
        "a run still going keeps its key"
    );
    assert!(
        without.exists(),
        "a young folder without a key waits its day"
    );
}

fn walk(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(at) = todo.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path.clone());
            }
            out.push(path);
        }
    }
    out
}

/// **A TLS pool's sign-in holds the engine to its pin, as SSH's holds the host key**: another
/// engine behind the same CA (a machine reinstalled, or another one given the CA's certificates)
/// is refused before the login is made, and a login made is refused once the engine changes.
#[test]
fn a_sign_in_over_tls_refuses_another_engine_behind_the_same_ca() {
    let pki = Pki::new();
    let through = Through {
        route: Route::parse("tcp://box").unwrap(),
        files: pki.files("client"),
        ca: pki.fingerprint("ca"),
        engine: Some("ENGINE-A".to_owned()),
    };
    let bin = tempfile::tempdir().unwrap();
    let id = bin.path().join("id");
    let docker = stand_in::program(
        bin.path(),
        "docker",
        &format!(
            "#!/bin/sh\nprintf '{{\"ID\":\"%s\"}}' \"$(cat '{}')\"\n",
            id.display()
        ),
    );
    let settings = tempfile::tempdir().unwrap();
    let docker = crate::dockerpool::Docker::at(docker)
        .with_settings_in(settings.path().join("docker-settings"))
        .over_tls(through);
    let sign_in = || {
        docker.sign_in_login(
            &crate::dockerpool::tests::made(),
            std::path::Path::new("/var/run/docker.sock"),
            &settings.path().join("sign-in"),
            "work",
            None,
        )
    };

    std::fs::write(&id, "ENGINE-B").unwrap();
    let refused = sign_in().unwrap_err();
    assert!(refused.contains("isn't the one you trusted"), "{refused}");

    std::fs::write(&id, "ENGINE-A").unwrap();
    let login = sign_in().unwrap();
    let pin = login.pin.as_ref().expect("a TLS login is pinned");
    assert_eq!(pin.holds(), Ok(()));
    std::fs::write(&id, "ENGINE-B").unwrap();
    let refused = pin.holds().unwrap_err();
    assert!(refused.contains("isn't the one you trusted"), "{refused}");
}
