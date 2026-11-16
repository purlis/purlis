#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **What curl and Node send a proxy, recorded** (T1.1-47): the inputs of the differential fuzz
// target `differential_host_parse_against_curl_and_node`. For each URL in `urls.txt`, curl and
// Node are pointed at a proxy that is a listener here, and the head each sends it is written to
// `<out>/<client>-<n>` as the URL, a newline, and the head: the target's input format.
//
//     node record.mjs <out>              curl and Node, through a listener on 127.0.0.1
//     node record.mjs <out> --no-listen  Node's `http:` requests alone, captured in-process
//
// `--no-listen` is for a sandbox that refuses to listen: Node's plain request is written to a
// stream of the recorder's own instead of a socket, which is byte for byte what it sends. A
// tunnel (`https:`) and curl need the listener, so CI records them fresh on every run.

import { execFile } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import http from "node:http";
import https from "node:https";
import net from "node:net";
import { dirname, join } from "node:path";
import process from "node:process";
import { Duplex } from "node:stream";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const urls = readFileSync(join(here, "urls.txt"), "utf8")
  .split("\n")
  .map((line) => line.replace(/^#.*/, "").trimEnd())
  .filter(Boolean);

/** The head a client sent on `socket`, up to its blank line, or what came before it closed. */
function headOn(socket) {
  return new Promise((resolve) => {
    let got = Buffer.alloc(0);
    const done = () => resolve(got.toString("latin1"));
    socket.on("data", (chunk) => {
      got = Buffer.concat([got, chunk]);
      const end = got.indexOf("\r\n\r\n");
      if (end >= 0) {
        got = got.subarray(0, end + 4);
        socket.destroy();
        done();
      }
    });
    socket.on("close", done);
    socket.on("error", done);
  });
}

/**
 * The head one client sends for one URL: a listener of its own on 127.0.0.1, given to `send` as
 * the proxy, which takes the first connection's head and answers nothing. A listener per client
 * per URL, so a head is never filed under another client's or another URL's request. The head is
 * kept however the client ends: it errors when the listener closes on it, after sending it.
 * Null when it sent none in time (it refused the URL).
 */
async function headSent(send) {
  const sockets = new Set();
  let first = null;
  const server = net.createServer((socket) => {
    sockets.add(socket);
    first ??= headOn(socket);
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  await send(`http://127.0.0.1:${server.address().port}`);
  const deadline = Date.now() + 500;
  while (first === null && Date.now() < deadline) await new Promise((r) => setTimeout(r, 10));
  const head = first === null ? null : await first;
  for (const socket of sockets) socket.destroy();
  server.close();
  return head || null;
}

function curl(url, proxy) {
  return new Promise((resolve) => {
    execFile(
      "curl",
      ["-s", "-o", "/dev/null", "--max-time", "3", "--globoff", "--proxy", proxy, "--noproxy", "", url],
      () => resolve(),
    );
  });
}

function node(url, proxy) {
  return new Promise((resolve) => {
    try {
      const secure = url.startsWith("https:");
      const agent = new (secure ? https : http).Agent({
        proxyEnv: secure ? { HTTPS_PROXY: proxy } : { HTTP_PROXY: proxy },
      });
      const req = (secure ? https : http).request(url, { agent, timeout: 3000 });
      // Ended either way: the listener closes on the head it read, so this errors.
      req.on("error", () => resolve());
      req.on("timeout", () => req.destroy());
      req.on("close", () => resolve());
      req.end();
    } catch {
      resolve();
    }
  });
}

/** Node's plain request for `url`, written to a stream of ours rather than a socket. */
function nodeInProcess(url) {
  return new Promise((resolve) => {
    let sent = "";
    const agent = new http.Agent({ proxyEnv: { HTTP_PROXY: "http://proxy.invalid:3128" } });
    agent.createConnection = () =>
      new Duplex({
        read() {},
        write(chunk, _encoding, callback) {
          sent += chunk.toString("latin1");
          if (sent.includes("\r\n\r\n")) resolve(sent);
          callback();
        },
      });
    try {
      const req = http.request(url, { agent });
      req.on("error", () => resolve(null));
      req.end();
    } catch {
      resolve(null);
    }
  });
}

/** The host and port a URL names, as a WHATWG URL client reads it: `[host, port]`, or null. */
function hostOf(text) {
  try {
    const url = new URL(text);
    const port = url.port || { "http:": "80", "https:": "443" }[url.protocol];
    return port ? `${url.hostname.toLowerCase()}:${port}` : null;
  } catch {
    return null;
  }
}

/** The host and port `head`'s request line names: the absolute form, or a CONNECT's authority. */
function namedOnRequestLine(head) {
  const target = head.split("\r\n")[0].split(" ")[1] ?? "";
  return hostOf(target.startsWith("http://") ? target : `https://${target}/`);
}

async function main([out, flag]) {
  if (!out) throw new Error("usage: node record.mjs <out> [--no-listen]");
  mkdirSync(out, { recursive: true });
  const written = { node: 0, curl: 0 };
  const mispaired = [];
  const write = (client, n, url, head) => {
    if (!head) return;
    // A recording is a head and the URL its client was asked for. One whose request line names
    // another host is a head filed under the wrong URL, which the differential target would
    // silently pass over: the recording fails instead.
    if (namedOnRequestLine(head) !== hostOf(url)) {
      mispaired.push(`${client}-${n}: ${url} -> ${head.split("\r\n")[0]}`);
    }
    writeFileSync(join(out, `${client}-${n}`), `${url}\n${head}`, "latin1");
    written[client] += 1;
  };
  if (flag === "--no-listen") {
    for (const [n, url] of urls.entries()) {
      if (url.startsWith("http:")) write("node", n, url, await nodeInProcess(url));
    }
  } else {
    for (const [n, url] of urls.entries()) {
      write("node", n, url, await headSent((proxy) => node(url, proxy)));
      write("curl", n, url, await headSent((proxy) => curl(url, proxy)));
    }
  }
  console.log(
    `recorded ${written.node + written.curl} heads (node ${written.node}, curl ${written.curl}) ` +
      `from ${urls.length} URLs into ${out}`,
  );
  if (mispaired.length > 0) {
    throw new Error(`heads whose request line names another host than their URL:\n${mispaired.join("\n")}`);
  }
  if (written.node === 0 || (flag !== "--no-listen" && written.curl === 0)) {
    throw new Error("a client recorded no heads at all");
  }
}

await main(process.argv.slice(2));
