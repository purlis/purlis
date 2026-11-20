// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (ADR 0128 D3a, D3c, D3h.10): a chat's authority is its own, its name
//! constraints are critical and are the chat's recorded hosts, a leaf is made for those hosts and
//! no other, and a client that asks for another name, or sends another `Host`, is not served.

use purlis_core::forge::proxy::listener::ChatCa;
use x509_cert::Certificate;
use x509_cert::der::Decode;
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::{BasicConstraints, NameConstraints};

use crate::support::forge_listener::{Answer, World, basic, tls};

fn der_of(pem: &str) -> Vec<u8> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    data_encoding::BASE64.decode(body.as_bytes()).unwrap()
}

const NAME_CONSTRAINTS: &str = "2.5.29.30";
const BASIC_CONSTRAINTS: &str = "2.5.29.19";

fn hosts() -> Vec<String> {
    vec!["api.github.com".into(), "github.com".into()]
}

#[test]
fn the_authority_has_critical_name_constraints_that_permit_the_hosts_and_exclude_every_address() {
    purlis_core::unsteered!();
    let ca = ChatCa::new(&hosts()).unwrap();
    let pem = ca.certificate_pem();
    assert!(
        pem.contains("BEGIN CERTIFICATE") && !pem.contains("PRIVATE KEY"),
        "{pem}"
    );
    let cert = Certificate::from_der(&der_of(pem)).unwrap();
    let extensions = cert
        .tbs_certificate()
        .extensions()
        .map(Vec::as_slice)
        .unwrap_or_default();

    let constraints = extensions
        .iter()
        .find(|e| e.extn_id.to_string() == NAME_CONSTRAINTS)
        .expect("name constraints");
    assert!(
        constraints.critical,
        "a constraint a client may skip is not one"
    );
    let parsed = NameConstraints::from_der(constraints.extn_value.as_bytes()).unwrap();
    let mut permitted: Vec<String> = parsed
        .permitted_subtrees
        .unwrap_or_default()
        .iter()
        .map(|t| match &t.base {
            GeneralName::DnsName(name) => name.to_string(),
            other => panic!("a permitted name that is not a DNS name: {other:?}"),
        })
        .collect();
    permitted.sort();
    assert_eq!(permitted, ["api.github.com", "github.com"]);
    let excluded = parsed.excluded_subtrees.unwrap_or_default();
    assert!(
        excluded
            .iter()
            .all(|t| matches!(t.base, GeneralName::IpAddress(_)))
            && excluded.len() == 2,
        "every IPv4 and IPv6 address is excluded: {excluded:?}"
    );

    let basic_constraints = extensions
        .iter()
        .find(|e| e.extn_id.to_string() == BASIC_CONSTRAINTS)
        .expect("basic constraints");
    let basic_constraints =
        BasicConstraints::from_der(basic_constraints.extn_value.as_bytes()).unwrap();
    assert!(basic_constraints.ca);
    assert_eq!(
        basic_constraints.path_len_constraint,
        Some(0),
        "it signs no other authority"
    );
}

#[test]
fn a_leaf_is_made_for_the_chats_hosts_to_the_letter_and_for_no_other_name() {
    purlis_core::unsteered!();
    let ca = ChatCa::new(&hosts()).unwrap();
    for host in ["github.com", "api.github.com", "GitHub.com", "github.com."] {
        assert!(ca.leaf_for(host).is_some(), "{host}");
    }
    for host in [
        "gist.github.com",
        "github.com.evil.example",
        "evilgithub.com",
        "evil.example",
        "gitlab.com",
        "codeload.github.com",
        "127.0.0.1",
        "",
    ] {
        assert!(ca.leaf_for(host).is_none(), "{host}");
        assert!(ca.server_config_for(host).is_none(), "{host}");
    }
    assert!(
        ChatCa::new(&[]).is_err(),
        "a chat with no forge host has no authority"
    );
    // One authority per chat: a second chat's is another key, and does not vouch for the first's.
    let other = ChatCa::new(&hosts()).unwrap();
    assert_ne!(ca.certificate_pem(), other.certificate_pem());
}

#[test]
fn a_client_that_trusts_the_authority_is_served_for_the_name_it_asked_for_and_only_that() {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    let request = format!(
        "GET /user HTTP/1.1\r\nHost: api.github.com\r\nAuthorization: {}\r\n\r\n",
        basic("x-access-token", w.sentinel.expose())
    );
    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    let reply = tls(
        &w,
        &ca,
        "api.github.com",
        443,
        "api.github.com",
        request.as_bytes(),
    )
    .expect("a handshake");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(w.fake.requests().len(), 1);

    // The `CONNECT` named one host and the client's SNI another: no certificate, no request.
    for sni in ["github.com", "gitlab.com", "evil.example"] {
        let result = tls(&w, &ca, "api.github.com", 443, sni, request.as_bytes());
        assert!(result.is_err(), "{sni} was served: {result:?}");
    }
    assert_eq!(w.fake.requests().len(), 1);
}

#[test]
fn a_request_inside_the_tunnel_that_names_another_host_is_refused() {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    let auth = basic("x-access-token", w.sentinel.expose());
    for host in [
        "gitlab.com",
        "api.github.com",
        "evil.example",
        "github.com:8443",
    ] {
        let request = format!(
            "GET /acme/app.git/info/refs?service=git-upload-pack HTTP/1.1\r\nHost: {host}\r\nAuthorization: {auth}\r\n\r\n"
        );
        let reply =
            tls(&w, &ca, "github.com", 443, "github.com", request.as_bytes()).expect("a handshake");
        assert_eq!(reply.status, 403, "{host}: {}", reply.text());
    }
    assert!(w.fake.requests().is_empty());
}

#[test]
fn a_fetch_through_the_tunnel_is_answered_for_a_recorded_repo_and_anonymously_for_a_public_one() {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    let auth = basic("x-access-token", w.sentinel.expose());
    let advert = "001e# service=git-upload-pack\n0000";
    w.fake.answer(Answer::ok(advert));
    let own = format!(
        "GET /acme/app.git/info/refs?service=git-upload-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {auth}\r\n\r\n"
    );
    let reply = tls(&w, &ca, "github.com", 443, "github.com", own.as_bytes()).unwrap();
    assert_eq!(reply.status, 200);
    let seen = w.fake.requests();
    assert_eq!(
        seen[0].url,
        "https://github.com/acme/app.git/info/refs?service=git-upload-pack"
    );
    assert!(
        seen[0]
            .headers
            .iter()
            .any(|(n, v)| n.eq_ignore_ascii_case("authorization") && v.starts_with("Basic ")),
        "{:?}",
        seen[0].headers
    );

    w.fake.answer(Answer::ok(advert));
    let public = format!(
        "GET /someone/public.git/info/refs?service=git-upload-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {auth}\r\n\r\n"
    );
    let reply = tls(&w, &ca, "github.com", 443, "github.com", public.as_bytes()).unwrap();
    assert_eq!(reply.status, 200);
    let seen = w.fake.requests();
    assert!(
        seen[1]
            .headers
            .iter()
            .all(|(n, _)| !n.eq_ignore_ascii_case("authorization")),
        "a public repo's fetch went with a credential: {:?}",
        seen[1].headers
    );
    // Not a push, and not a dumb-protocol file.
    for target in [
        "/someone/public.git/git-receive-pack",
        "/someone/public.git/HEAD",
        "/someone/public.git/objects/info/packs",
    ] {
        let method = if target.ends_with("receive-pack") {
            "POST"
        } else {
            "GET"
        };
        let request =
            format!("{method} {target} HTTP/1.1\r\nHost: github.com\r\nContent-Length: 0\r\n\r\n");
        let reply = tls(&w, &ca, "github.com", 443, "github.com", request.as_bytes()).unwrap();
        assert_eq!(reply.status, 403, "{target}");
    }
}

fn batch_answer(href: &str, extra_actions: &str) -> Answer {
    let body = format!(
        r#"{{"transfer":"basic","objects":[{{"oid":"abc","size":10,"actions":{{"download":{{"href":"{href}","header":{{"Authorization":"RemoteAuth storage-secret-1"}}}}{extra_actions}}}}}]}}"#
    );
    Answer {
        status: 200,
        headers: vec![("Content-Type".into(), "application/vnd.git-lfs+json".into())],
        body: body.into_bytes(),
    }
}

fn lfs_batch(w: &World, ca: &ChatCa, host: &str) -> crate::support::forge_listener::Reply {
    let body =
        r#"{"operation":"download","transfers":["basic"],"objects":[{"oid":"abc","size":10}]}"#;
    let request = format!(
        "POST /acme/app.git/info/lfs/objects/batch HTTP/1.1\r\nHost: {host}\r\nAuthorization: {}\r\nAccept: application/vnd.git-lfs+json\r\nContent-Type: application/vnd.git-lfs+json\r\nContent-Length: {}\r\n\r\n{body}",
        basic("x-access-token", w.sentinel.expose()),
        body.len()
    );
    tls(w, ca, host, 443, host, request.as_bytes()).expect("a handshake")
}

#[test]
fn an_lfs_batch_answer_is_written_again_so_the_chat_holds_no_storage_address_and_no_storage_credential()
 {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    let storage = "https://github-cloud.githubusercontent.com/alambic/media/abc?sig=1";
    w.fake.answer(batch_answer(storage, ""));
    let reply = lfs_batch(&w, &ca, "github.com");
    assert_eq!(reply.status, 200, "{}", reply.text());
    let text = reply.text();
    assert!(
        !text.contains("github-cloud") && !text.contains("storage-secret"),
        "{text}"
    );
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let href = doc["objects"][0]["actions"]["download"]["href"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        href.starts_with("https://github.com/__purlis-lfs/"),
        "{href}"
    );
    assert!(
        doc["objects"][0]["actions"]["download"]
            .get("header")
            .is_none(),
        "{text}"
    );
    // The forge was sent the batch as the person.
    assert!(
        w.fake.requests()[0]
            .headers
            .iter()
            .any(|(n, v)| n.eq_ignore_ascii_case("authorization") && v.starts_with("Basic "))
    );

    // The chat asks for the object at the address it was given: the listener fetches the one it
    // holds, with the headers that came with it, and none of the person's.
    w.fake.answer(Answer {
        status: 200,
        headers: vec![("Content-Type".into(), "application/octet-stream".into())],
        body: b"OBJECTDATA".to_vec(),
    });
    let path = href.strip_prefix("https://github.com").unwrap();
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\nRange: bytes=0-4\r\n\r\n",
        basic("x-access-token", w.sentinel.expose())
    );
    let object = tls(&w, &ca, "github.com", 443, "github.com", request.as_bytes()).unwrap();
    assert_eq!(object.status, 200);
    assert_eq!(object.body, b"OBJECTDATA");
    let fetched = &w.fake.requests()[1];
    assert_eq!(fetched.url, storage);
    let header = |name: &str| {
        fetched
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(header("authorization"), ["RemoteAuth storage-secret-1"]);
    assert_eq!(header("range"), ["bytes=0-4"]);
    let sent = format!("{fetched:?}");
    assert!(
        !sent.contains(w.sentinel.expose()) && !sent.contains(crate::support::forge_listener::REAL),
        "{sent}"
    );

    // A token that was never given, one given on another host, and a storage address a chat names
    // itself are refused, and nothing is fetched.
    let before = w.fake.requests().len();
    for (host, target) in [
        (
            "github.com",
            "/__purlis-lfs/00000000000000000000000000000000".to_string(),
        ),
        ("github.com", "/__purlis-lfs/../x".to_string()),
        ("api.github.com", path.to_string()),
        ("github.com", format!("{path}x")),
    ] {
        let request = format!("GET {target} HTTP/1.1\r\nHost: {host}\r\n\r\n");
        let reply = tls(&w, &ca, host, 443, host, request.as_bytes()).unwrap();
        assert_eq!(reply.status, 403, "{host}{target}: {}", reply.text());
    }
    assert_eq!(w.fake.requests().len(), before);
}

#[test]
fn an_lfs_batch_that_names_a_host_off_the_list_or_an_action_that_is_not_a_download_is_not_passed_on()
 {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    for answer in [
        batch_answer("https://evil.example/object", ""),
        batch_answer("http://github-cloud.githubusercontent.com/o", ""),
        batch_answer("https://169.254.169.254/latest", ""),
        batch_answer("https://github-cloud.githubusercontent.com:8443/o", ""),
        batch_answer(
            "https://github-cloud.githubusercontent.com/o",
            r#","upload":{"href":"https://github-cloud.githubusercontent.com/u"}"#,
        ),
        batch_answer(
            "https://github-cloud.githubusercontent.com/o",
            r#","verify":{"href":"https://github.com/verify"}"#,
        ),
    ] {
        w.fake.answer(answer);
        let reply = lfs_batch(&w, &ca, "github.com");
        assert_eq!(reply.status, 502, "{}", reply.text());
        assert!(
            reply
                .text()
                .contains("LFS files live on a host purlis does not know"),
            "{}",
            reply.text()
        );
        assert!(!reply.text().contains("evil.example") && !reply.text().contains("storage-secret"));
    }
}

#[test]
fn a_git_request_with_no_credential_is_challenged_so_git_asks_its_helper_and_a_wrong_one_is_refused()
 {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    let target =
        "GET /acme/app.git/info/refs?service=git-upload-pack HTTP/1.1\r\nHost: github.com\r\n";
    // `git` sends its first request with none, and only a 401 makes it ask for one.
    let none = tls(
        &w,
        &ca,
        "github.com",
        443,
        "github.com",
        format!("{target}\r\n").as_bytes(),
    )
    .unwrap();
    assert_eq!(none.status, 401, "{}", none.text());
    assert!(
        none.header("www-authenticate")
            .is_some_and(|v| v.starts_with("Basic ")),
        "{none:?}"
    );
    // A credential that is not this chat's is not a challenge: it is refused.
    let wrong = format!(
        "{target}Authorization: {}\r\n\r\n",
        basic("x-access-token", "not-the-sentinel")
    );
    let wrong = tls(&w, &ca, "github.com", 443, "github.com", wrong.as_bytes()).unwrap();
    assert_eq!(wrong.status, 403, "{}", wrong.text());
    assert!(w.fake.requests().is_empty());
    // And with the helper's answer the request goes through.
    w.fake
        .answer(Answer::ok("001e# service=git-upload-pack\n0000"));
    let right = format!(
        "{target}Authorization: {}\r\n\r\n",
        basic("x-access-token", w.sentinel.expose())
    );
    let right = tls(&w, &ca, "github.com", 443, "github.com", right.as_bytes()).unwrap();
    assert_eq!(right.status, 200);
}
