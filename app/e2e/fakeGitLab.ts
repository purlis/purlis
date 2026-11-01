// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync, spawn } from "node:child_process";
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer as createHttp, type IncomingMessage, type ServerResponse } from "node:http";
import { createServer as createHttps } from "node:https";
import type { AddressInfo } from "node:net";
import { dirname, join } from "node:path";
import type { Duplex } from "node:stream";
import { gunzipSync } from "node:zlib";

/**
 * **A self-hosted GitLab on this machine, for a spec of the real window's clones**: no external
 * network, and every byte of git goes over a socket through the machine's real git and the app's
 * real credential helper.
 *
 * - **Git over smart HTTP, behind TLS** (a clone URL is `https://`, whatever the build), through
 *   git's own `http-backend`. Every request needs a token this forge granted: without one it
 *   answers 401, as a GitLab does for a private project.
 * - **The OAuth side of a browser sign-in**, on plain loopback HTTP, which only a test build's
 *   form may post to. It holds the PKCE exchange to its challenge. {@link FakeGitLab.playBrowser}
 *   is the person's browser pressing Authorize.
 * - **The way a repo is served** is the repo's own: `slowly` drips its pack over a number of
 *   seconds, and `silently` sends nothing once the fetch is asked for.
 *
 * The app is told about it by the file `THE_RUNS_FAKE_FORGE` names, and git by the run's global
 * config ({@link gitConfigFor}): the host name resolves to loopback for git alone, and its
 * certificate is trusted for that server alone.
 */

/** How a repo's pack is served. */
export type Served = { way: "whole" } | { way: "slowly"; seconds: number } | { way: "silently" };

/** One request git made, as the forge saw it. */
export type Heard = { repo: string; signedIn: boolean; user: string };

export type FakeGitLab = {
  /** The server, as an account and a clone URL name it: its host and its port. */
  server: string;
  port: number;
  /** The root of the OAuth side. */
  oauth: string;
  /** Its certificate, which git is told to trust. */
  certificate: string;
  /** Makes a bare repo of `files` files named `name`, served `served`. */
  addRepo(name: string, files: number, served: Served): void;
  /** Every git request so far, oldest first. */
  heard(): Heard[];
  /** How many tokens the OAuth side has granted. */
  grants(): number;
  /** Takes a token as if the person had made one in the forge's settings. */
  acceptToken(token: string): void;
  /** Stops accepting any token granted or accepted so far. */
  revokeEveryToken(): void;
  /** Presses Authorize on the authorization `page`: the browser is sent back to the app. */
  playBrowser(page: string): Promise<void>;
  /** The HTTPS address of a repo. */
  webUrl(name: string): string;
  /** Lets every response still open go, then closes. */
  stop(): Promise<void>;
};

/** The owner every repo is under. */
export const OWNER = "acme";

/** The `-c`-free git every repo is made with: never the machine's config or signer. */
function git(cwd: string, args: string[]): void {
  execFileSync("git", ["-c", "commit.gpgsign=false", ...args], {
    cwd,
    env: {
      ...process.env,
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_AUTHOR_NAME: "fixture",
      GIT_AUTHOR_EMAIL: "fixture@example.invalid",
      GIT_COMMITTER_NAME: "fixture",
      GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    },
    stdio: ["ignore", "ignore", "ignore"],
  });
}

/** A certificate for `host` alone, with its key, from the machine's own openssl. */
function certificateFor(dir: string, host: string): { key: string; cert: string } {
  const key = join(dir, "key.pem");
  const cert = join(dir, "cert.pem");
  execFileSync(
    "openssl",
    [
      "req",
      "-x509",
      "-newkey",
      "rsa:2048",
      "-nodes",
      "-days",
      "2",
      "-subj",
      `/CN=${host}`,
      "-addext",
      `subjectAltName=DNS:${host}`,
      "-keyout",
      key,
      "-out",
      cert,
    ],
    { stdio: ["ignore", "ignore", "ignore"] },
  );
  return { key, cert };
}

/** Everything a request's body holds. */
async function bodyOf(request: IncomingMessage): Promise<Buffer> {
  const parts: Buffer[] = [];
  for await (const part of request) parts.push(part as Buffer);
  return Buffer.concat(parts);
}

/** Whether a body git sent asks for objects: `want` is what no ref listing says. */
function asksForObjects(request: IncomingMessage, body: Buffer): boolean {
  const read = request.headers["content-encoding"] === "gzip" ? gunzipSync(body) : body;
  return read.includes("want ");
}

/** The CGI answer's head, split from its body: `[status, headers, body]`. */
function splitCgi(out: Buffer): [number, Record<string, string>, Buffer] {
  const end = out.indexOf("\r\n\r\n");
  const headers: Record<string, string> = {};
  let status = 200;
  for (const line of out.subarray(0, end).toString().split("\r\n")) {
    const at = line.indexOf(":");
    const name = line.slice(0, at).toLowerCase();
    const value = line.slice(at + 1).trim();
    if (name === "status") status = Number.parseInt(value, 10);
    else headers[name] = value;
  }
  return [status, headers, out.subarray(end + 4)];
}

/**
 * Starts the forge on `host`, keeping what it makes under `dir`. The app sees it once the file
 * `namedIn` holds it, and writes the page a browser sign-in would open to `page`.
 */
export async function startFakeGitLab(
  dir: string,
  host: string,
  namedIn: string,
  page: string,
): Promise<FakeGitLab> {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const repos = join(dir, "repos");
  const certificate = certificateFor(dir, host);
  const served = new Map<string, Served>();
  const tokens = new Set<string>();
  const log: Heard[] = [];
  const codes = new Map<string, { challenge: string; redirect: string }>();
  const open = new Set<Duplex>();
  const timers = new Set<NodeJS.Timeout>();
  let grants = 0;

  /** Who a request's `Authorization` says it is, when that is a token this forge took. */
  const signedInAs = (request: IncomingMessage): string | undefined => {
    const [scheme, encoded] = (request.headers.authorization ?? "").split(" ");
    if (scheme !== "Basic" || encoded === undefined) return undefined;
    const said = Buffer.from(encoded, "base64").toString();
    const at = said.indexOf(":");
    return tokens.has(said.slice(at + 1)) ? said.slice(0, at) : undefined;
  };

  /** Sends `body` over `seconds`, a slice each quarter of a second. */
  const drip = (response: ServerResponse, body: Buffer, seconds: number) => {
    const slices = Math.max(1, seconds * 4);
    let sent = 0;
    let slice = 0;
    const timer = setInterval(() => {
      slice += 1;
      const until = Math.floor((body.length * slice) / slices);
      if (response.destroyed) {
        clearInterval(timer);
        timers.delete(timer);
        return;
      }
      response.write(body.subarray(sent, until));
      sent = until;
      if (slice >= slices) {
        clearInterval(timer);
        timers.delete(timer);
        response.end();
      }
    }, 250);
    timers.add(timer);
  };

  /** `git http-backend` answers the request, as a forge's own smart HTTP does. */
  const backend = (
    request: IncomingMessage,
    response: ServerResponse,
    body: Buffer,
    path: string,
    query: string,
    way: Served,
  ) => {
    const child = spawn("git", ["http-backend"], {
      env: {
        PATH: process.env.PATH,
        GIT_CONFIG_GLOBAL: "/dev/null",
        GIT_CONFIG_NOSYSTEM: "1",
        GIT_PROJECT_ROOT: repos,
        GIT_HTTP_EXPORT_ALL: "1",
        GIT_PROTOCOL: String(request.headers["git-protocol"] ?? ""),
        PATH_INFO: path,
        QUERY_STRING: query,
        REQUEST_METHOD: request.method ?? "GET",
        CONTENT_TYPE: request.headers["content-type"] ?? "",
        CONTENT_LENGTH: String(body.length),
        HTTP_CONTENT_ENCODING: request.headers["content-encoding"] ?? "",
        REMOTE_USER: "signed-in",
      },
      stdio: ["pipe", "pipe", "ignore"],
    });
    child.stdin.on("error", () => undefined);
    child.stdin.end(body);
    const out: Buffer[] = [];
    child.stdout.on("data", (part: Buffer) => out.push(part));
    child.stdout.on("end", () => {
      const [status, headers, answer] = splitCgi(Buffer.concat(out));
      response.writeHead(status, headers);
      if (way.way === "slowly" && answer.length > 64 * 1024) drip(response, answer, way.seconds);
      else response.end(answer);
    });
  };

  const https = createHttps(
    {
      key: readFileSync(certificate.key),
      cert: readFileSync(certificate.cert),
    },
    (request, response) => {
      void (async () => {
        const url = new URL(request.url ?? "/", `https://${host}`);
        const repo = /^\/[^/]+\/([^/]+)\.git\//.exec(url.pathname)?.[1] ?? "";
        const user = signedInAs(request);
        log.push({ repo, signedIn: user !== undefined, user: user ?? "" });
        const body = await bodyOf(request);
        if (user === undefined) {
          response.writeHead(401, { "WWW-Authenticate": 'Basic realm="GitLab"' });
          response.end("HTTP Basic: Access denied\n");
          return;
        }
        const way = served.get(repo);
        if (way === undefined) {
          response.writeHead(404);
          response.end();
          return;
        }
        // The forge has gone silent: it took the fetch and never says another word.
        if (way.way === "silently" && asksForObjects(request, body)) return;
        backend(request, response, body, url.pathname, url.search.slice(1), way);
      })();
    },
  );
  https.on("connection", (socket) => {
    open.add(socket);
    socket.on("close", () => open.delete(socket));
  });

  const http = createHttp((request, response) => {
    void (async () => {
      const form = new URLSearchParams((await bodyOf(request)).toString());
      const answer = (status: number, said: unknown) => {
        response.writeHead(status, { "Content-Type": "application/json" });
        response.end(JSON.stringify(said));
      };
      if (request.url !== "/oauth/token" || form.get("grant_type") !== "authorization_code") {
        answer(400, { error: "unsupported_grant_type" });
        return;
      }
      const held = codes.get(form.get("code") ?? "");
      const verifier = form.get("code_verifier") ?? "";
      const digest = Buffer.from(createHash("sha256").update(verifier).digest("base64url"));
      const challenge = Buffer.from(held?.challenge ?? "");
      const right =
        held !== undefined &&
        form.get("redirect_uri") === held.redirect &&
        challenge.length === digest.length &&
        timingSafeEqual(challenge, digest);
      if (!right) {
        answer(400, { error: "invalid_grant" });
        return;
      }
      codes.delete(form.get("code") ?? "");
      const token = randomBytes(24).toString("hex");
      tokens.add(token);
      grants += 1;
      answer(200, {
        access_token: token,
        refresh_token: randomBytes(24).toString("hex"),
        token_type: "bearer",
        expires_in: 7200,
        scope: "api",
      });
    })();
  });
  http.on("connection", (socket) => {
    open.add(socket);
    socket.on("close", () => open.delete(socket));
  });

  const listening = (server: typeof https | typeof http) =>
    new Promise<number>((resolve) =>
      server.listen(0, "127.0.0.1", () => resolve((server.address() as AddressInfo).port)),
    );
  const port = await listening(https);
  const oauthPort = await listening(http);
  const server = `${host}:${port}`;
  const oauth = `http://127.0.0.1:${oauthPort}`;
  writeFileSync(namedIn, JSON.stringify({ host: server, oauth, page }));

  return {
    server,
    port,
    oauth,
    certificate: certificate.cert,
    addRepo(name, files, way) {
      const work = join(dir, "work", name);
      mkdirSync(work, { recursive: true });
      // Random, so nothing compresses it away and a pack's bytes come with its objects.
      for (let n = 0; n < files; n += 1)
        writeFileSync(join(work, `file-${n}.bin`), randomBytes(8 * 1024));
      git(work, ["init", "-q", "-b", "main"]);
      git(work, ["add", "-A"]);
      git(work, ["commit", "-q", "-m", name]);
      const bare = join(repos, OWNER, `${name}.git`);
      mkdirSync(dirname(bare), { recursive: true });
      git(dirname(bare), ["clone", "-q", "--bare", work, bare]);
      served.set(name, way);
    },
    heard: () => [...log],
    grants: () => grants,
    acceptToken: (token) => void tokens.add(token),
    revokeEveryToken: () => tokens.clear(),
    async playBrowser(address) {
      const asked = new URL(address);
      if (asked.host !== server) throw new Error(`the page is on ${asked.host}, not ${server}`);
      const redirect = asked.searchParams.get("redirect_uri") ?? "";
      if (
        asked.pathname !== "/oauth/authorize" ||
        asked.searchParams.get("response_type") !== "code" ||
        asked.searchParams.get("code_challenge_method") !== "S256"
      )
        throw new Error("the page does not ask for a PKCE authorization code");
      const code = randomBytes(16).toString("hex");
      codes.set(code, { challenge: asked.searchParams.get("code_challenge") ?? "", redirect });
      const back = new URL(redirect);
      back.searchParams.set("code", code);
      back.searchParams.set("state", asked.searchParams.get("state") ?? "");
      const came = await fetch(back);
      if (!came.ok) throw new Error(`the app answered the redirect with ${came.status}`);
    },
    webUrl: (name) => `https://${server}/${OWNER}/${name}`,
    async stop() {
      for (const timer of timers) clearInterval(timer);
      for (const socket of open) socket.destroy();
      rmSync(namedIn, { force: true });
      await Promise.all([
        new Promise((resolve) => https.close(resolve)),
        new Promise((resolve) => http.close(resolve)),
      ]);
    },
  };
}

/**
 * What git's global config needs to reach `forge`: its host name resolved to loopback, and its
 * certificate trusted for that server alone.
 */
export function gitConfigFor(forge: FakeGitLab): string {
  const [host] = forge.server.split(":");
  return [
    "[http]",
    `\tcurloptResolve = ${host}:${forge.port}:127.0.0.1`,
    `[http "https://${forge.server}"]`,
    `\tsslCAInfo = ${forge.certificate}`,
    "",
  ].join("\n");
}
