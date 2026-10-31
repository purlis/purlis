// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `purlis listen`'s Node shim, tried for real (#2019): a server started on the chat's port, by
// Node's own `http`, by Vite and by Next.js, listens on the chat's socket and on no TCP port,
// answers there, and says it listens on the port. Run by CI under each Node it names:
//
//   node tests/listen-shim/check.mjs plain|vite|next
//   node tests/listen-shim/check.mjs dial-plain|dial-express|dial-vite|dial-refused
//
// The `dial-` kinds try its dial mode (#3316): with no socket, the server dials in to a hook
// socket this script stands in for, and this script takes each parked connection as purlis's
// door does, writes the go line, waits for it back, and carries one request over it. A Vite
// dev server's hot reload upgrades to its WebSocket over one. Another port of the same server is
// left as it was, and where the hook socket refuses, the server listens as it would without the
// shim.
//
// Vite's and Next.js's packages are installed from npm into a temporary project, at the versions
// pinned below.

import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync, lstatSync, rmSync } from "node:fs";
import { request } from "node:http";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const SHIM = resolve(import.meta.dirname, "../../crates/purlis-cli/src/listen/shim.cjs");
const VERSIONS = { vite: "5.4.11", next: "14.2.15", react: "18.3.1", express: "4.21.2" };
const given = process.argv[2];
const dialled = given.startsWith("dial-");
const kind = dialled ? given.slice("dial-".length) : given;
const port = 20000 + Math.floor(Math.random() * 20000);
const dir = mkdtempSync(join(tmpdir(), "listen-shim-"));
const socket = join(dir, `${port}.sock`);
const seen = [];

function fail(why) {
  console.error(`FAIL (${given}, Node ${process.version}): ${why}\n${seen.join("")}`);
  process.exitCode = 1;
}

function project() {
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture", private: true }));
  if (kind === "plain" || kind === "refused") {
    writeFileSync(
      join(dir, "server.cjs"),
      `const http = require("node:http");
const server = http.createServer((req, res) => res.end("${kind} ok"));
server.listen(Number(process.env.PORT), "0.0.0.0", () => {
  console.log("address " + JSON.stringify(server.address()));
});
// Another port of the same program, which the shim leaves as it is.
const other = http.createServer((req, res) => res.end("other ok"));
other.listen(Number(process.env.PORT) + 1, "127.0.0.1");`,
    );
    return ["node", ["server.cjs"]];
  }
  if (kind === "express") {
    execFileSync("npm", ["install", "--no-audit", "--no-fund", `express@${VERSIONS.express}`], {
      cwd: dir,
      stdio: "inherit",
    });
    writeFileSync(
      join(dir, "server.cjs"),
      `const express = require("express");
const app = express();
app.get("/", (req, res) => res.send("express ok"));
const server = app.listen(Number(process.env.PORT), () => {
  console.log("address " + JSON.stringify(server.address()));
});`,
    );
    return ["node", ["server.cjs"]];
  }
  if (kind === "vite") {
    execFileSync("npm", ["install", "--no-audit", "--no-fund", `vite@${VERSIONS.vite}`], {
      cwd: dir,
      stdio: "inherit",
    });
    writeFileSync(join(dir, "index.html"), "<!doctype html><title>vite ok</title><p>vite ok</p>");
    return ["node", ["node_modules/vite/bin/vite.js", "--port", String(port), "--strictPort"]];
  }
  if (kind === "next") {
    execFileSync(
      "npm",
      [
        "install",
        "--no-audit",
        "--no-fund",
        `next@${VERSIONS.next}`,
        `react@${VERSIONS.react}`,
        `react-dom@${VERSIONS.react}`,
      ],
      { cwd: dir, stdio: "inherit" },
    );
    execFileSync("mkdir", ["-p", join(dir, "pages")]);
    writeFileSync(join(dir, "pages", "index.js"), "export default () => 'next ok';");
    return ["node", ["node_modules/next/dist/bin/next", "dev", "-p", String(port)]];
  }
  throw new Error(`no fixture ${kind}`);
}

function ask() {
  return new Promise((done) => {
    const req = request(
      { socketPath: socket, path: "/", headers: { Host: `localhost:${port}` }, timeout: 120000 },
      (res) => {
        let body = "";
        res.on("data", (chunk) => (body += chunk));
        res.on("end", () => done({ status: res.statusCode, body }));
      },
    );
    req.on("error", (err) => done({ status: 0, body: String(err) }));
    req.end();
  });
}

function tcpListens() {
  return new Promise((done) => {
    const client = connect({ host: "127.0.0.1", port }, () => {
      client.destroy();
      done(true);
    });
    client.on("error", () => done(false));
  });
}

if (dialled) {
  await dialCheck();
  process.exit();
}

const [program, args] = project();
const server = spawn(program, args, {
  cwd: dir,
  detached: true,
  env: {
    ...process.env,
    NODE_OPTIONS: `--require "${SHIM}"`,
    PURLIS_SERVE_SOCKET: socket,
    PURLIS_SERVE_PORT: String(port),
    PORT: String(port),
    HOST: "127.0.0.1",
    NEXT_TELEMETRY_DISABLED: "1",
  },
});
server.stdout.on("data", (chunk) => seen.push(String(chunk)));
server.stderr.on("data", (chunk) => seen.push(String(chunk)));

try {
  const deadline = Date.now() + 180000;
  while (!existsSync(socket) && Date.now() < deadline) {
    await new Promise((wait) => setTimeout(wait, 250));
  }
  if (!existsSync(socket) || !lstatSync(socket).isSocket()) {
    fail(`no socket at ${socket}`);
  } else {
    const answer = await ask();
    if (answer.status !== 200) fail(`the socket answered ${answer.status}: ${answer.body}`);
    else if (!answer.body.includes(`${kind} ok`)) fail(`the page was not the fixture's: ${answer.body}`);
    if (await tcpListens()) fail(`something listens on TCP port ${port}`);
    // What the server printed, without its colours.
    const said = seen.join("").replace(/\x1b\[[0-9;]*m/g, "");
    if (kind === "plain" && !said.includes(`"port":${port}`)) fail(`address() did not say the port`);
    if (kind === "vite" && !said.includes(`localhost:${port}`)) fail(`Vite did not print the port`);
  }
} finally {
  try {
    process.kill(-server.pid, "SIGKILL");
  } catch {
    // Gone already.
  }
  rmSync(dir, { recursive: true, force: true });
}
if (process.exitCode !== 1) console.log(`ok (${kind}, Node ${process.version})`);

/**
 * The dial-mode check (#3316): this script is the chat's hook socket and its door.
 */
async function dialCheck() {
  const { createServer } = await import("node:net");
  const hook = join(dir, "hooks.sock");
  const token = "0123456789abcdef";
  const go = '{"go":true}\n';
  const parked = [];
  const asked = [];
  const app = createServer((slot) => {
    let held = "";
    const onData = (chunk) => {
      held += chunk;
      const end = held.indexOf("\n");
      if (end < 0) return;
      slot.removeListener("data", onData);
      let ask = {};
      try {
        ask = JSON.parse(held.slice(0, end));
      } catch {
        // Not a line this reads.
      }
      asked.push(ask);
      if (kind === "refused" || ask.chat !== 7 || ask.serve !== port || ask.token !== token) {
        slot.end(`${JSON.stringify({ taken: false, refused: "no door" })}\n`);
        return;
      }
      slot.write('{"taken":true}\n');
      parked.push(slot);
    };
    slot.on("data", onData);
    slot.on("error", () => {});
  });
  await new Promise((done) => app.listen(hook, done));

  /** As the door does: one parked connection, told go, answered go, then `request` over it. */
  const carry = (request, until) =>
    new Promise((done) => {
      const slot = parked.shift();
      if (!slot) return done("no connection was parked");
      let said = "";
      let went = false;
      slot.on("data", (chunk) => {
        said += chunk;
        if (!went) {
          if (!said.startsWith(go)) return;
          went = true;
          said = said.slice(go.length);
          slot.write(request);
        }
        if (until && until(said)) {
          slot.destroy();
          done(said);
        }
      });
      slot.on("close", () => done(said));
      slot.write(go);
      setTimeout(() => {
        slot.destroy();
        done(said || "nothing answered");
      }, 60000).unref();
    });

  const [program, args] = project();
  const env = { ...process.env };
  delete env.PURLIS_SERVE_SOCKET;
  const server = spawn(program, args, {
    cwd: dir,
    detached: true,
    env: {
      ...env,
      NODE_OPTIONS: `--require "${SHIM}"`,
      PURLIS_SERVE_DIAL: hook,
      PURLIS_SERVE_PORT: String(port),
      PURLIS_CHAT: "7",
      PURLIS_CHAT_TOKEN: token,
      PORT: String(port),
      HOST: "127.0.0.1",
    },
  });
  server.stdout.on("data", (chunk) => seen.push(String(chunk)));
  server.stderr.on("data", (chunk) => seen.push(String(chunk)));
  try {
    const deadline = Date.now() + 180000;
    const ready = () => (kind === "refused" ? asked.length > 0 : parked.length >= 2);
    while (!ready() && Date.now() < deadline) {
      await new Promise((wait) => setTimeout(wait, 250));
    }
    if (kind === "refused") {
      // Refused: it listens as it would without the shim (here, unsandboxed, it may).
      while (!(await tcpListens()) && Date.now() < deadline) {
        await new Promise((wait) => setTimeout(wait, 250));
      }
      if (!(await tcpListens())) fail("refused, the server did not listen as it would without the shim");
      return;
    }
    if (!ready()) return fail("the server parked no connection");
    if (asked.some((ask) => ask.chat !== 7 || ask.serve !== port || ask.token !== token)) {
      fail(`a serve line was not the chat's: ${JSON.stringify(asked)}`);
    }
    const page = await carry(
      `GET / HTTP/1.1\r\nHost: localhost:${port}\r\nConnection: close\r\n\r\n`,
    );
    if (!page.startsWith("HTTP/1.1 200")) fail(`the page did not come back: ${page}`);
    else if (!page.includes(`${kind} ok`) && !page.includes("vite ok")) {
      fail(`the page was not the fixture's: ${page}`);
    }
    // The connection taken was dialled again.
    const again = Date.now() + 10000;
    while (parked.length < 2 && Date.now() < again) {
      await new Promise((wait) => setTimeout(wait, 100));
    }
    const second = await carry(
      `GET / HTTP/1.1\r\nHost: localhost:${port}\r\nConnection: close\r\n\r\n`,
    );
    if (!second.startsWith("HTTP/1.1 200")) fail(`a second request was not served: ${second}`);
    if (kind === "vite") {
      const switched = await carry(
        [
          "GET / HTTP/1.1",
          `Host: localhost:${port}`,
          "Connection: Upgrade",
          "Upgrade: websocket",
          "Sec-WebSocket-Version: 13",
          "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
          "Sec-WebSocket-Protocol: vite-hmr",
          "",
          "",
        ].join("\r\n"),
        (said) => said.includes("\r\n\r\n"),
      );
      if (!switched.startsWith("HTTP/1.1 101")) fail(`hot reload's WebSocket did not upgrade: ${switched}`);
    }
    if (await tcpListens()) fail(`something listens on TCP port ${port}`);
    const said = seen.join("").replace(/\x1b\[[0-9;]*m/g, "");
    if (kind !== "vite" && !said.includes(`"port":${port}`)) fail("address() did not say the port");
    if (kind === "vite" && !said.includes(`localhost:${port}`)) fail("Vite did not print the port");
    if (kind === "plain") {
      const other = await new Promise((done) => {
        const req = request(
          { host: "127.0.0.1", port: port + 1, path: "/", timeout: 10000 },
          (res) => {
            let body = "";
            res.on("data", (chunk) => (body += chunk));
            res.on("end", () => done(body));
          },
        );
        req.on("error", (err) => done(String(err)));
        req.end();
      });
      if (other !== "other ok") fail(`another port was not left as it was: ${other}`);
    }
  } finally {
    try {
      process.kill(-server.pid, "SIGKILL");
    } catch {
      // Gone already.
    }
    app.close();
    rmSync(dir, { recursive: true, force: true });
    if (process.exitCode !== 1) console.log(`ok (${given}, Node ${process.version})`);
  }
}
