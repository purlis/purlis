# Fuzzing the parsers

The bytes a sandboxed chat controls meet a parser first: its proxy's request head and SOCKS
handshake, the host patterns every decision asks, the JSON `purlis hook` reads, and the heads a
browser sends a chat's front door and its reach proxy. Each has a [cargo-fuzz] target in
`crates/purlis-core/fuzz/` (T1.1-47, #6894). A target fails on a panic and on an assertion of its
own: beyond reading anything without crashing, each asserts what its parser must keep true, such
as "a head the proxy carries names one host, the one it judges and dials".

| Target | What it reads |
| --- | --- |
| `fuzz_proxy_request_head` | The egress proxy's request line and headers (`egress::Request::parse`) |
| `fuzz_connect_authority` | A `CONNECT`'s `host:port` |
| `fuzz_socks_handshake` | The SOCKS5 greeting and request (`egress::socks_request`) |
| `fuzz_host_pattern` | `egress::allows` and `allows_on`, against `hosts::Host`'s reading of a policy entry |
| `fuzz_hook_stdin` | A hook's JSON, through each core reader `purlis hook` hands it to |
| `fuzz_door_host_origin` | The door's `Host`, `Origin` and cookies (`door::judge`) |
| `fuzz_browser_reach_request_head` | The browser reach proxy's head, `Host` check and site |
| `differential_host_parse_against_curl_and_node` | The proxy's host against the host curl and Node meant for the same URL |

The targets reach the core's private parsers through `purlis_core::sandbox::fuzzing`, built only
with the `fuzzing` feature, which only the fuzz crate turns on.

## The differential target

For a URL, the proxy must read the host the client meant, or refuse the request. The reference is
the `url` crate, which implements the WHATWG URL Standard that Node implements, and the heads curl
and Node really send: `fuzz/clients/record.mjs` points both at a listener for each URL in
`fuzz/clients/urls.txt` (userinfo, a final dot, punycode, IP encodings, delimiters). Node's plain
requests are committed as seeds; CI records curl and Node fresh on its runner every run.

What it found, and what became of each:

- **Fixed:** the proxy read a host with a byte no URL host has (`1.2.3.4<tab>.github.com`,
  `evil.test@x.example.com`, `a?.github.com`, `[name]`) and a host that ends in a number but is no
  IPv4 address (`0x7f.1`, `2130706433`, which curl, Node and the resolver all read as 127.0.0.1).
  `egress::Request::parse` and the SOCKS port now refuse both. None reached another host: the name
  dialled was the one judged, macOS's resolver and glibc's read none of them as an address, and an
  unlisted one was already refused by the policy's reading (`hosts::Host::parse`).
- **Not a disagreement:** a URL the proxy refuses (a host with a character WHATWG allows but no
  DNS name has, such as `a!b.example`). Refusing is not reading another host.

## Running

cargo-fuzz 0.13.2, on the pinned stable toolchain, with no sanitizer (purlis-core is safe Rust, so
a fault is a panic, which libFuzzer catches as it is):

```sh
cargo install cargo-fuzz --locked --version 0.13.2
cd crates/purlis-core
cargo fuzz build --sanitizer none
# Fuzz one target for a minute; new inputs go to fuzz/corpus/ (ignored), the seeds are read.
cargo fuzz run --sanitizer none fuzz_proxy_request_head fuzz/corpus/fuzz_proxy_request_head \
  fuzz/seeds/fuzz_proxy_request_head -- -max_total_time=60
# Replay the committed seeds once, as CI does.
cargo fuzz run --sanitizer none fuzz_proxy_request_head fuzz/seeds/fuzz_proxy_request_head -- -runs=0
```

A crash writes its input to `fuzz/artifacts/<target>/`. Turn it into a test of the parser it
broke, next to that parser's tests, and add the input to `fuzz/seeds/<target>/`. A crash that means
an escape (a refused host reached, a refused file written) is filed at once as a public issue
labelled `area:security` and `priority:p0`, and fixed first (Q301).

## In CI

- **Every pull request** that changes `crates/purlis-core/` (or the toolchain, the lock file or
  the workflow) runs `.github/workflows/fuzz.yml`: the targets built, every seed and the runner's
  fresh curl and Node recordings replayed once.
- **Nightly**, the entry `fuzz-parsers` fuzzes each target from its seeds for a time box
  (`.github/actions/fuzz-parsers` with `seconds`).

[cargo-fuzz]: https://github.com/rust-fuzz/cargo-fuzz
