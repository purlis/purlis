// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `purlis listen`'s Node shim, tried for real (#2019): a server started on the chat's port, by
// Node's own `http`, by Vite and by Next.js, listens on the chat's socket and on no TCP port,
// answers there, and says it listens on the port. Run by CI under each Node it names:
//
//   node tests/listen-shim/check.mjs plain|vite|next
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
const VERSIONS = { vite: "5.4.11", next: "14.2.15", react: "18.3.1" };
const kind = process.argv[2];
const port = 20000 + Math.floor(Math.random() * 20000);
const dir = mkdtempSync(join(tmpdir(), "listen-shim-"));
const socket = join(dir, `${port}.sock`);
const seen = [];

function fail(why) {
  console.error(`FAIL (${kind}, Node ${process.version}): ${why}\n${seen.join("")}`);
  process.exitCode = 1;
}

function project() {
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "fixture", private: true }));
  if (kind === "plain") {
    writeFileSync(
      join(dir, "server.cjs"),
      `const http = require("node:http");
const server = http.createServer((req, res) => res.end("plain ok"));
server.listen(Number(process.env.PORT), "0.0.0.0", () => {
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
