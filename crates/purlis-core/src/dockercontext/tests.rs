// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What `docker context ls --format '{{json .}}'` prints, as Docker 24 to 27 write it.
const LISTED: &str = r#"{"Current":false,"Description":"Current DOCKER_HOST based configuration","DockerEndpoint":"unix:///var/run/docker.sock","Error":"","Name":"default"}
{"Current":true,"Description":"","DockerEndpoint":"ssh://me@build-box","Error":"","Name":"build-box"}
{"Current":false,"Description":"","DockerEndpoint":"tcp://lab.example:2376","Error":"","Name":"lab"}
{"Current":false,"Description":"","DockerEndpoint":"tcp://10.0.0.9:2375","Error":"","Name":"open"}
{"Current":false,"Description":"","DockerEndpoint":"tcp://lab.example:2376","Error":"","Name":"lab-unverified"}
{"Current":false,"Description":"Docker Desktop","DockerEndpoint":"unix:///Users/me/.docker/run/docker.sock","Error":"","Name":"desktop-linux"}
"#;

/// What `docker context inspect` prints for those names.
const INSPECTED: &str = r#"[
    {
        "Name": "default",
        "Metadata": {},
        "Endpoints": {"docker": {"Host": "unix:///var/run/docker.sock", "SkipTLSVerify": false}},
        "TLSMaterial": {},
        "Storage": {"MetadataPath": "<IN MEMORY>", "TLSPath": "<IN MEMORY>"}
    },
    {
        "Name": "build-box",
        "Metadata": {},
        "Endpoints": {"docker": {"Host": "ssh://me@build-box", "SkipTLSVerify": false}},
        "TLSMaterial": {},
        "Storage": {
            "MetadataPath": "/Users/me/.docker/contexts/meta/1f2e",
            "TLSPath": "/Users/me/.docker/contexts/tls/1f2e"
        }
    },
    {
        "Name": "lab",
        "Metadata": {},
        "Endpoints": {"docker": {"Host": "tcp://lab.example:2376", "SkipTLSVerify": false}},
        "TLSMaterial": {"docker": ["ca.pem", "cert.pem", "key.pem"]},
        "Storage": {
            "MetadataPath": "/Users/me/.docker/contexts/meta/9a8b",
            "TLSPath": "/Users/me/.docker/contexts/tls/9a8b"
        }
    },
    {
        "Name": "open",
        "Metadata": {},
        "Endpoints": {"docker": {"Host": "tcp://10.0.0.9:2375", "SkipTLSVerify": false}},
        "TLSMaterial": {},
        "Storage": {
            "MetadataPath": "/Users/me/.docker/contexts/meta/7c6d",
            "TLSPath": "/Users/me/.docker/contexts/tls/7c6d"
        }
    },
    {
        "Name": "lab-unverified",
        "Metadata": {},
        "Endpoints": {"docker": {"Host": "tcp://lab.example:2376", "SkipTLSVerify": true}},
        "TLSMaterial": {"docker": ["cert.pem", "key.pem"]},
        "Storage": {
            "MetadataPath": "/Users/me/.docker/contexts/meta/5e4f",
            "TLSPath": "/Users/me/.docker/contexts/tls/5e4f"
        }
    },
    {
        "Name": "desktop-linux",
        "Metadata": {"Description": "Docker Desktop"},
        "Endpoints": {"docker": {"Host": "unix:///Users/me/.docker/run/docker.sock", "SkipTLSVerify": false}},
        "TLSMaterial": {},
        "Storage": {
            "MetadataPath": "/Users/me/.docker/contexts/meta/3a2b",
            "TLSPath": "/Users/me/.docker/contexts/tls/3a2b"
        }
    }
]"#;

fn named<'a>(contexts: &'a [DockerContext], name: &str) -> &'a DockerContext {
    contexts
        .iter()
        .find(|one| one.name == name)
        .unwrap_or_else(|| panic!("no context {name} in {contexts:?}"))
}

#[test]
fn an_ssh_context_is_read_into_the_ssh_route_its_endpoint_names_and_marked_current() {
    let read = contexts_in(LISTED, INSPECTED);
    let ssh = named(&read, "build-box");
    assert!(ssh.current);
    assert_eq!(ssh.endpoint.route().as_deref(), Some("ssh://me@build-box"));
    assert!(matches!(ssh.endpoint, Endpoint::Ssh(_)));
    assert_eq!(
        read.iter().map(|one| one.name.as_str()).collect::<Vec<_>>(),
        [
            "default",
            "build-box",
            "lab",
            "open",
            "lab-unverified",
            "desktop-linux"
        ],
        "in the order ls lists them"
    );
}

#[test]
fn a_tls_context_is_read_into_the_tcp_route_with_the_paths_of_its_own_tls_material() {
    let read = contexts_in(LISTED, INSPECTED);
    let lab = named(&read, "lab");
    assert!(!lab.current);
    assert_eq!(
        lab.endpoint.route().as_deref(),
        Some("tcp://lab.example:2376")
    );
    let files = lab.endpoint.files().expect("its TLS files");
    let folder = Path::new("/Users/me/.docker/contexts/tls/9a8b/docker");
    assert_eq!(files.ca, folder.join("ca.pem"));
    assert_eq!(files.cert, folder.join("cert.pem"));
    assert_eq!(files.key_file(), Some(folder.join("key.pem").as_path()));
}

#[test]
fn a_local_socket_context_is_this_machine_and_names_no_route() {
    let read = contexts_in(LISTED, INSPECTED);
    for name in ["default", "desktop-linux"] {
        assert_eq!(named(&read, name).endpoint, Endpoint::Here, "{name}");
        assert_eq!(named(&read, name).endpoint.route(), None);
    }
}

#[test]
fn plain_tcp_and_tls_that_does_not_verify_the_server_are_refused_in_adr_0123s_words() {
    let read = contexts_in(LISTED, INSPECTED);
    assert_eq!(
        named(&read, "open").endpoint,
        Endpoint::Refused(crate::sshroute::PLAIN_TCP.to_owned())
    );
    assert_eq!(
        named(&read, "lab-unverified").endpoint,
        Endpoint::Refused(crate::tlsroute::NO_VERIFY.to_owned())
    );
    assert_eq!(named(&read, "open").endpoint.route(), None);
}

#[test]
fn a_tls_context_without_a_client_certificate_is_refused_with_what_it_lacks() {
    let inspected = INSPECTED.replace(
        r#""TLSMaterial": {"docker": ["ca.pem", "cert.pem", "key.pem"]}"#,
        r#""TLSMaterial": {"docker": ["ca.pem"]}"#,
    );
    let read = contexts_in(LISTED, &inspected);
    let Endpoint::Refused(why) = &named(&read, "lab").endpoint else {
        panic!("not refused: {:?}", named(&read, "lab"));
    };
    assert!(why.contains("client certificate"), "{why}");
}

#[test]
fn a_context_inspect_does_not_describe_or_that_is_not_json_is_left_out() {
    let read = contexts_in(LISTED, "[]");
    assert!(read.is_empty());
    assert!(contexts_in("not json\n", INSPECTED).is_empty());
    assert!(contexts_in(LISTED, "not json").is_empty());
    // A name no context could have is left out, as a Podman connection's is.
    let listed = format!(
        "{}\n{LISTED}",
        r#"{"Current":false,"Name":"bad\u0007name"}"#
    );
    assert_eq!(contexts_in(&listed, INSPECTED).len(), 6);
}

#[test]
fn a_context_whose_tls_folder_is_not_an_absolute_path_is_refused() {
    let inspected = INSPECTED.replace("/Users/me/.docker/contexts/tls/9a8b", "<IN MEMORY>");
    let read = contexts_in(LISTED, &inspected);
    assert!(matches!(named(&read, "lab").endpoint, Endpoint::Refused(_)));
}

#[test]
fn a_context_that_now_names_another_route_has_moved_and_one_that_still_names_it_has_not() {
    let read = contexts_in(LISTED, INSPECTED);
    assert_eq!(moved(&read, "build-box", "ssh://me@build-box"), None);
    // The pool's route has its socket written in; the context's has none, and is the same.
    assert_eq!(
        moved(&read, "build-box", "ssh://me@build-box/var/run/docker.sock"),
        None
    );
    assert_eq!(
        moved(&read, "build-box", "ssh://me@old-box/var/run/docker.sock"),
        Some("ssh://me@build-box".to_owned())
    );
    assert_eq!(moved(&read, "lab", "tcp://lab.example:2376"), None);
    assert_eq!(
        moved(&read, "lab", "tcp://old-lab.example:2376"),
        Some("tcp://lab.example:2376".to_owned())
    );
    // Gone, or now naming this machine or something refused: nothing to follow.
    assert_eq!(moved(&read, "gone", "ssh://me@build-box"), None);
    assert_eq!(moved(&read, "desktop-linux", "ssh://me@build-box"), None);
    assert_eq!(moved(&read, "open", "ssh://me@build-box"), None);
}

/// A context `ls` lists with an error is not asked of `inspect`, so one broken context never
/// hides the others.
#[test]
fn a_context_listed_with_an_error_is_not_inspected() {
    let listed = format!(
        "{LISTED}{}\n",
        r#"{"Current":false,"DockerEndpoint":"","Error":"context \"broken\": meta.json is not valid","Name":"broken"}"#
    );
    let names = contexts_listed(&listed);
    assert!(!names.iter().any(|name| name == "broken"), "{names:?}");
    assert_eq!(names.len(), 6);
}
