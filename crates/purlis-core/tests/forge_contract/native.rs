// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The recorded run over the native transport (ADR 0070 §7, FW-2a): each recording's exchanges
//! served as HTTP by a `wiremock` server, and the backend sending its requests through
//! [`purlis_core::forge::http::Http`] with a sign-in token.
//!
//! An exchange is served once, to the request `gh api` would have sent for its `Call`: the
//! same method, path and query, and the same JSON body. That request is worked out here from
//! the recording alone ([`expected`]), not by the transport's own code, so a transport that
//! dropped a query or misread a field fails the case. A reply with exit 0 is a `200` with the recorded answer. A request
//! nobody recorded is a `404`, which fails the case, and [`Over`]'s check fails on an exchange
//! nobody asked for. The CLI route is a transport that refuses to answer, so a case that
//! reached for the CLI fails too.

use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::http::{ApiRoot, TokenSource};
use purlis_core::forge::recorded::Exchange;
use purlis_core::forge::route::{HostScope, Resolver, SignIn};
use purlis_core::forge::transport::{Call, Endpoint, Field, NoAnswer, Reply, Transport};
use purlis_core::forge::{Account, Caller, Forge, ForgeError};
use secrecy::SecretString;
use serde_json::Value;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::Over;

/// The token every native case signs in with.
pub const TOKEN: &str = "native-contract-token-5f2a";

struct Fixed;
impl TokenSource for Fixed {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(SecretString::from(TOKEN))
    }
}

/// The CLI route of a native run: it never answers.
pub struct NoCli;
impl Transport for NoCli {
    fn send(&self, _forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        Err(NoAnswer::Missing(format!(
            "a native case reached for the CLI: {}",
            call.path()
        )))
    }
    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        Err(ForgeError::transport("a native case reached for the CLI"))
    }
}

/// One exchange's request, as the native transport sends it.
struct Exactly {
    kind: Kind,
    method: http::Method,
    path_and_query: String,
    body: Option<Value>,
    /// A call with a file: the file's name and bytes, sent in a form rather than as JSON.
    form: Option<(String, Vec<u8>)>,
}

impl wiremock::Match for Exactly {
    fn matches(&self, request: &Request) -> bool {
        let mut asked = request.url.path().to_string();
        if let Some(query) = request.url.query() {
            asked.push('?');
            asked.push_str(query);
        }
        if let Some((name, bytes)) = &self.form {
            let kind = request
                .headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            let holds = |needle: &[u8]| request.body.windows(needle.len()).any(|w| w == needle);
            return request.method == self.method
                && request.url.path() == self.path_and_query
                && kind.starts_with("multipart/form-data; boundary=")
                && holds(format!("filename=\"{name}\"").as_bytes())
                && holds(bytes)
                && request
                    .headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    == Some(&format!("Bearer {TOKEN}"));
        }
        let body = if request.body.is_empty() {
            None
        } else {
            serde_json::from_slice::<Value>(&request.body).ok()
        };
        let token = request
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());
        let header = |name: &str| request.headers.get(name).and_then(|v| v.to_str().ok());
        // Each forge is asked in its own dialect: GitHub's media type and API version, and
        // plain JSON with no GitHub header for GitLab.
        let dialect = match self.kind {
            Kind::GitHub => {
                header("accept") == Some("application/vnd.github+json")
                    && header("x-github-api-version") == Some("2022-11-28")
            }
            Kind::GitLab => {
                header("accept") == Some("application/json")
                    && header("x-github-api-version").is_none()
            }
        };
        dialect
            && request.method == self.method
            && asked == self.path_and_query
            && body == self.body
            && token == Some(&format!("Bearer {TOKEN}"))
    }
}

/// The JSON a field is, as `gh api` sends it: `-f` a string, `-F` a boolean, null or integer
/// when it reads as one.
fn json_of(field: &Field) -> (String, Value) {
    match field {
        Field::Text(name, value) => (name.clone(), Value::String(value.clone())),
        Field::Typed(name, value) => {
            let v = match value.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                v => v
                    .parse::<i64>()
                    .map(Value::from)
                    .unwrap_or_else(|_| Value::String(v.into())),
            };
            (name.clone(), v)
        }
        Field::File(..) => panic!("a file is sent in a form, never as JSON"),
    }
}

/// A write's JSON body: each field a key, and a `name[]` field one more element of list `name`,
/// as `gh api` builds it.
fn body_of(fields: &[Field]) -> serde_json::Map<String, Value> {
    let mut body = serde_json::Map::new();
    for field in fields {
        let (name, value) = json_of(field);
        // `a[][k]`: key `k` of the last object of list `a`, a new one where it has `k`.
        if let Some((list, key)) = name
            .split_once("[][")
            .and_then(|(list, rest)| Some((list, rest.strip_suffix(']')?)))
        {
            let items = body
                .entry(list.to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("a list field");
            if items.last().is_none_or(|last| last.get(key).is_some()) {
                items.push(Value::Object(serde_json::Map::new()));
            }
            items
                .last_mut()
                .and_then(Value::as_object_mut)
                .expect("a list of objects")
                .insert(key.to_string(), value);
            continue;
        }
        match name.strip_suffix("[]") {
            Some(list) => body
                .entry(list.to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("a list field")
                .push(value),
            None => {
                body.insert(name, value);
            }
        }
    }
    body
}

/// The HTTP request a recorded call is, read from the recording.
fn expected(kind: Kind, call: &Call) -> Exactly {
    if let [Field::File(_, name, content)] = call.fields.as_slice() {
        use base64::Engine as _;
        let Endpoint::Rest {
            method: Some(method),
            path,
        } = &call.endpoint
        else {
            panic!("a file goes in a write");
        };
        return Exactly {
            kind,
            method: method.word().parse().unwrap(),
            path_and_query: format!("/{path}"),
            body: None,
            form: Some((
                name.clone(),
                base64::engine::general_purpose::STANDARD
                    .decode(content)
                    .unwrap(),
            )),
        };
    }
    assert!(
        call.fields.iter().all(|f| {
            let name = match f {
                Field::Text(name, _) | Field::Typed(name, _) => name.clone(),
                Field::File(..) => panic!("a file is a call's one field"),
            };
            let flat = name
                .split_once("[][")
                .map_or(name.as_str(), |(list, _)| list);
            !flat.trim_end_matches("[]").contains('[')
        }),
        "a recording here names no nested field but a list, or a list of objects"
    );
    match &call.endpoint {
        Endpoint::Graphql => {
            let (query, rest) = call.fields.split_first().expect("a GraphQL query");
            let variables: serde_json::Map<String, Value> = rest.iter().map(json_of).collect();
            Exactly {
                kind,
                method: http::Method::POST,
                path_and_query: "/graphql".into(),
                body: Some(serde_json::json!({"query": json_of(query).1, "variables": variables})),
                form: None,
            }
        }
        Endpoint::Rest { method: None, path } => {
            assert!(
                call.fields.is_empty(),
                "a recorded GET carries its query in its path"
            );
            Exactly {
                kind,
                method: http::Method::GET,
                path_and_query: format!("/{path}"),
                body: None,
                form: None,
            }
        }
        Endpoint::Rest {
            method: Some(method),
            path,
        } => Exactly {
            kind,
            method: method.word().parse().unwrap(),
            path_and_query: format!("/{path}"),
            body: Some(Value::Object(body_of(&call.fields))),
            form: None,
        },
    }
}

/// The account a native case signs in as, on `host`.
pub fn account(kind: Kind, host: &str) -> Account {
    Account {
        kind,
        host: host.into(),
        login: "octocat".into(),
    }
}

/// `kind`'s backend on `host` over the native transport, against a server answering
/// `recording`. `host` is the instance the account is on: github.com, gitlab.com, or a
/// self-managed GitLab, whose API root differs and whose capabilities charter has not asked.
pub fn over(kind: &str, host: &str, recording: &str) -> Over {
    let kind = Kind::parse(kind).expect("a forge kind");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(MockServer::start());
    let root = ApiRoot::at(&server.uri());
    let file: Value = serde_json::from_str(recording).unwrap();
    for exchange in file["exchanges"].as_array().cloned().unwrap_or_default() {
        let exchange: Exchange = serde_json::from_value(exchange).unwrap();
        assert_eq!(
            exchange.reply.code, 0,
            "a native recording replays answers only"
        );
        let mock = Mock::given(expected(kind, &exchange.call))
            .respond_with(ResponseTemplate::new(200).set_body_string(exchange.reply.out))
            .up_to_n_times(1)
            .expect(1);
        rt.block_on(mock.mount(&server));
    }
    let resolver = Resolver::new(kind, host)
        .at_root(root)
        .cli_over(Arc::new(NoCli))
        .signed_in(
            &HostScope::for_a_test(),
            account(kind, host),
            SignIn {
                tokens: Arc::new(Fixed),
                imported_from_cli: false,
            },
        );
    let mut forge = Forge::default_of(kind);
    forge.host = host.to_string();
    let backend = forge.backend_for(Arc::new(resolver));
    Over {
        backend,
        caller: Caller::window().as_account(account(kind, host)),
        host: host.to_string(),
        scene: super::Scene::recorded(kind),
        check: Box::new(move || rt.block_on(server.verify())),
        sweep: Box::new(|| Ok(Vec::new())),
    }
}
