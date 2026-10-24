# The confinement of a studio extension, as #2777 built it

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-10-24**, ticket #2777 (SK-14). §2 and §3 said what a confined extension reads, writes and reaches, and that its proxy refuses hosts that serve a model. This file records how the confinement was built, what its proxy checks, and what it does not stop.

## What it reads

A confined program reads its own folder, its state folder, its own temporary folder and the system's programs. Of a program prefix (`/usr`, `/usr/local`, Homebrew's on Apple silicon and on Intel, the Command Line Tools'), it reads the program parts alone: `bin`, `sbin`, `lib`, `libexec`, `share`, `include`, `opt`, `Cellar` and `Frameworks`. It never reads a prefix's `etc` or `var`. On macOS the data volume (`/System/Volumes/Data`) is held out of the system's folders, so no home or project is read under that second name. Of each folder above what it reads, it may see the folder itself and nothing in it. On macOS it may also list `/`, which a program needs to start. It reads no other process's arguments or environment through the system's settings (`kern.proc*`).

## What its proxy carries

- **Only its declared hosts**, less any host that serves a model, checked at each question (§3).
- **Only to a public address.** No connection reaches a private, shared or special-use address (RFC 1918, RFC 6598, `fc00::/7` and the rest of RFC 6890). The check is made after each lookup, at every connect, and covers an IPv6 address that carries an IPv4 one.
- **A tunnel only where its TLS ClientHello names the host it was asked for.** The proxy reads the first TLS record, never decrypts anything, and refuses a tunnel whose server name is missing or names another host.
- **No SOCKS request.**
- Its two ports stay purlis's own, and stay bound, for the rest of the run. No later listener on this machine is given them.

## What it does not stop

- **A declared host can pass on what it is sent.** Any declared host is a path out for what the extension reads, by design.
- **The server name is the client's to say.** A host that answers with a model provider's service under its own name, or a CDN that routes by something other than the server name, is not told apart. Neither is a name pinned to a provider's address. The proxy holds the name to the tunnel. It cannot tell what serves it. #3094 follows this up.
- **A program that leaves its process group** can outlive its question, inside the same sandbox. The prompt says so.

**Built by #2777.**
