// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The seams: stand-ins for the slices of ADR 0128 that have not merged yet.**
//
// The scenario in `run.mjs` talks only to what this file exports, and to real `gh`, `glab` and
// `git`. Each slice, when it merges, replaces its part here with the product's, and its TODO
// marker goes with it:
//
//   TODO(slice A) the forge proxy: the listeners, the sentinel swap, the default-deny list, the
//                 push rule, the hold. Here a small one in this file, with a short list.
//   TODO(slice B) the chat's wiring: the environment, the `gh` wrapper, the credential helper.
//                 Here they are written to a folder by `makeChat`.
//   TODO(slice C) the credential source: the keyring and renewal. Here a Map in memory and a
//                 refresh before expiry and once on a 401.
//   TODO(slice D) the readiness check and the project record. Here `recordProject`.
//   TODO(slice F) held forge acts. Here a list of asks and `approve`.
//   TODO(slice G) the sign-in. Here the device flow, against the fake forge.
//
// Nothing here is a model of what the product must do beyond what ADR 0128 says (D3 to D8); where
// a stand-in does less than the ADR (the allowlist is short, GraphQL is judged by the reader in
// `graphql-lite.mjs`), the marker says so.

import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import path from 'node:path';
import tls from 'node:tls';
import zlib from 'node:zlib';
import { describe } from './graphql-lite.mjs';
import { run, tool } from './harness.mjs';

export const REFUSED = 'purlis refused this: it is not something a chat may do for you.';
export const HELD = 'purlis holds merges. It has asked the person; the merge happens when they approve it in Asks.';

const ZERO = '0'.repeat(40);
const DEFAULT_BRANCHES = new Set(['main', 'master']);
const MERGE_ROOTS = new Set(['mergePullRequest', 'enablePullRequestAutoMerge']);
const ALLOWED_MUTATIONS = new Set(['createPullRequest', 'addComment']);
const HOP = new Set(['expect', 'connection', 'proxy-connection', 'keep-alive', 'transfer-encoding', 'upgrade', 'te', 'trailer', 'host', 'content-length']);
const STRIPPED = new Set(['authorization', 'private-token', 'x-http-method-override', 'x-method-override', 'x-forwarded-for', 'x-forwarded-host', 'forwarded', 'x-original-url', 'x-rewrite-url']);

const readBody = (req) =>
  new Promise((resolve) => {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', () => resolve(Buffer.concat(chunks)));
  });

/** A request's own credentials: Basic's password, a token or bearer value, a private token. */
function presented(headers) {
  const out = [];
  const auth = headers.authorization ?? '';
  if (/^basic /i.test(auth)) {
    const decoded = Buffer.from(auth.slice(6), 'base64').toString();
    out.push(decoded.slice(decoded.indexOf(':') + 1));
  } else if (auth) out.push(auth.replace(/^(token|bearer) /i, ''));
  if (headers['private-token']) out.push(headers['private-token']);
  return out;
}

function sameSecret(a, b) {
  const x = Buffer.from(String(a));
  const y = Buffer.from(String(b));
  return x.length === y.length && crypto.timingSafeEqual(x, y);
}

/** Path segments, decoded once; a dot segment, an empty one, a backslash or a NUL refuses the request. */
function segmentsOf(pathname) {
  if (!pathname.startsWith('/')) return null;
  const parts = pathname.slice(1).split('/');
  const out = [];
  for (const raw of parts) {
    let seg;
    try {
      seg = decodeURIComponent(raw);
    } catch {
      return null;
    }
    if (seg === '.' || seg === '..' || seg.includes('\\') || seg.includes('\0') || (seg === '' && raw !== '')) return null;
    out.push(seg);
  }
  if (out.at(-1) === '') out.pop(); // a trailing slash
  return out.some((s) => s === '') ? null : out;
}

/** The ref commands that begin a push: `[{ old, new, ref }]`, and whether it carries push options. */
function pushCommands(body) {
  const buf = body.length > 2 && body[0] === 0x1f && body[1] === 0x8b ? zlib.gunzipSync(body) : body;
  const cmds = [];
  let options = false;
  let i = 0;
  while (i + 4 <= buf.length) {
    const len = parseInt(buf.subarray(i, i + 4).toString(), 16);
    if (len === 0) break;
    const line = buf.subarray(i + 4, i + len).toString();
    const [text, caps = ''] = line.split('\0');
    if (caps.split(' ').includes('push-options')) options = true;
    const m = /^([0-9a-f]{40}) ([0-9a-f]{40}) (\S+)\n?$/.exec(text);
    if (!m) return { cmds: [], options, unreadable: true };
    cmds.push({ old: m[1], new: m[2], ref: m[3] });
    i += len;
  }
  return { cmds, options, unreadable: cmds.length === 0 };
}

/**
 * @param {{ work: string, ca: ReturnType<import('./harness.mjs').makeCa>, clock: { now(): number },
 *           forges: Record<'github' | 'gitlab', any> }} o
 */
export async function createApp({ work, ca, clock, forges }) {
  const sentinel = `sentinel-${crypto.randomBytes(24).toString('base64url')}`; // TODO(slice A): per chat, 128 bits, constant-time compare
  const sockDir = fs.mkdtempSync(path.join(work, 's-'));
  fs.chmodSync(sockDir, 0o700);
  const sockPath = path.join(sockDir, 'p.sock');

  const byHost = new Map(Object.values(forges).map((f) => [f.host, f]));
  const keyring = new Map(); // TODO(slice C): the OS keyring. host -> { access, refresh, expiresAt, account }
  const recorded = new Map(); // TODO(slice D): `${host}/${owner}/${name}` -> { host, owner, name, id, forge }
  const created = new Set(); // branches the chat created: `${host}/${owner}/${name}#${branch}` (D3f)
  const asks = []; // TODO(slice F): held acts
  const audit = [];
  const refreshes = []; // `${host}: before expiry` or `${host}: after a 401`

  // ---- talking to a forge as the app itself ----------------------------------------------------

  function upstream(forge, { method = 'GET', pathname, headers = {}, body = null }) {
    return new Promise((resolve, reject) => {
      const buf = body == null ? null : Buffer.isBuffer(body) ? body : Buffer.from(typeof body === 'string' ? body : JSON.stringify(body));
      const req = https.request(
        { host: '127.0.0.1', port: forge.port, method, path: pathname, headers: { host: forge.host, ...headers, ...(buf ? { 'content-length': buf.length } : {}) }, ca: ca.pemText, servername: 'localhost', agent: false },
        (res) => {
          const chunks = [];
          res.on('data', (c) => chunks.push(c));
          res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(chunks) }));
        },
      );
      req.on('error', reject);
      req.end(buf);
    });
  }
  const form = (o) => ({ body: new URLSearchParams(o).toString(), headers: { 'content-type': 'application/x-www-form-urlencoded', accept: 'application/json' } });
  const CLIENT_ID = 'fake-client-id';
  const paths = (f) =>
    f.kind === 'github'
      ? { device: '/login/device/code', token: '/login/oauth/access_token' }
      : { device: '/oauth/authorize_device', token: '/oauth/token' };

  // ---- the sign-in (TODO(slice G)) -------------------------------------------------------------

  /** The device flow, once per forge host: the person's press is `forge.approve`; the token goes to the keyring only. */
  async function signIn(forge, { approve = (code) => forge.approve(code) } = {}) {
    const p = paths(forge);
    const dev = JSON.parse((await upstream(forge, { method: 'POST', pathname: p.device, ...form({ client_id: CLIENT_ID, scope: forge.kind === 'github' ? 'repo read:org' : 'api' }) })).body);
    approve(dev.user_code);
    for (let i = 0; i < 20; i += 1) {
      const r = await upstream(forge, { method: 'POST', pathname: p.token, ...form({ client_id: CLIENT_ID, device_code: dev.device_code, grant_type: 'urn:ietf:params:oauth:grant-type:device_code' }) });
      const t = JSON.parse(r.body);
      if (t.access_token) {
        keyring.set(forge.host, { access: t.access_token, refresh: t.refresh_token ?? null, expiresAt: t.expires_in ? clock.now() + t.expires_in * 1000 : null });
        const me = JSON.parse((await asPerson(forge, { pathname: forge.kind === 'github' ? '/user' : '/api/v4/user' })).body);
        keyring.get(forge.host).account = me.login ?? me.username;
        return keyring.get(forge.host).account;
      }
      if (t.error !== 'authorization_pending') throw new Error(`sign-in failed: ${t.error}`);
      await new Promise((r2) => setTimeout(r2, 50));
    }
    throw new Error('sign-in timed out');
  }

  // ---- the credential source: renewal (TODO(slice C)) -------------------------------------------

  const renewing = new Map();
  async function renew(forge, why) {
    if (renewing.has(forge.host)) return renewing.get(forge.host);
    const p = (async () => {
      const k = keyring.get(forge.host);
      if (!k?.refresh) throw new Error(`no refresh token for ${forge.host}: the person signs in again`);
      const r = await upstream(forge, { method: 'POST', pathname: paths(forge).token, ...form({ client_id: CLIENT_ID, grant_type: 'refresh_token', refresh_token: k.refresh }) });
      const t = JSON.parse(r.body);
      if (!t.access_token) throw new Error(`renewal refused: ${t.error}`);
      keyring.set(forge.host, { ...k, access: t.access_token, refresh: t.refresh_token ?? null, expiresAt: t.expires_in ? clock.now() + t.expires_in * 1000 : null });
      refreshes.push(`${forge.host}: ${why}`);
    })().finally(() => renewing.delete(forge.host));
    renewing.set(forge.host, p);
    return p;
  }
  /** The access token to use now: renewed first when it is within a minute of its end. */
  async function accessFor(forge) {
    const k = keyring.get(forge.host);
    if (!k) throw new Error(`not signed in to ${forge.host}`);
    if (k.expiresAt != null && clock.now() >= k.expiresAt - 60_000) await renew(forge, 'before expiry');
    return keyring.get(forge.host).access;
  }
  async function asPerson(forge, { method = 'GET', pathname, body = null, git = false }) {
    const call = async () => {
      const tok = await accessFor(forge);
      const auth = git ? `Basic ${Buffer.from(`x-access-token:${tok}`).toString('base64')}` : `Bearer ${tok}`;
      return upstream(forge, { method, pathname, body, headers: { authorization: auth, ...(body ? { 'content-type': 'application/json' } : {}) } });
    };
    let r = await call();
    if (r.status === 401 && keyring.get(forge.host)?.refresh) {
      await renew(forge, 'after a 401');
      r = await call();
    }
    return r;
  }

  // ---- the project record (TODO(slice D)) ------------------------------------------------------

  const keyOf = (host, owner, name) => `${host}/${owner}/${name}`.toLowerCase();
  async function recordProject(forge, owner, name) {
    const r = await asPerson(forge, { pathname: forge.kind === 'github' ? `/repos/${owner}/${name}` : `/api/v4/projects/${encodeURIComponent(`${owner}/${name}`)}` });
    if (r.status !== 200) throw new Error(`${owner}/${name} is not reachable as ${keyring.get(forge.host)?.account}: ${r.status}`);
    const repo = JSON.parse(r.body);
    const rec = { host: forge.host, owner, name, id: repo.id, forge: forge.kind };
    recorded.set(keyOf(forge.host, owner, name), rec);
    return rec;
  }

  // ---- held acts (TODO(slice F)) ----------------------------------------------------------------

  async function raiseMerge(forge, rec, number, via) {
    if (asks.some((a) => a.host === forge.host && a.repo === `${rec.owner}/${rec.name}` && a.number === number && !a.done)) return;
    // What the ask shows is read from the forge as the person, never from the chat's request.
    const r = await asPerson(forge, { pathname: forge.kind === 'github' ? `/repos/${rec.owner}/${rec.name}/pulls/${number}` : `/api/v4/projects/${rec.id}/merge_requests/${number}` });
    const pr = JSON.parse(r.body);
    asks.push({ kind: 'merge', via, host: forge.host, forge, repo: `${rec.owner}/${rec.name}`, rec, number, title: pr.title, headSha: forge.kind === 'github' ? pr.head?.sha : pr.sha, done: false });
  }
  /** The person's press on Merge: the app merges as the person, naming the head they were shown. */
  async function approve(ask) {
    const { forge, rec, number } = ask;
    const r =
      forge.kind === 'github'
        ? await asPerson(forge, { method: 'PUT', pathname: `/repos/${rec.owner}/${rec.name}/pulls/${number}/merge`, body: { sha: ask.headSha, merge_method: 'merge' } })
        : await asPerson(forge, { method: 'PUT', pathname: `/api/v4/projects/${rec.id}/merge_requests/${number}/merge`, body: { sha: ask.headSha } });
    ask.done = true;
    return { status: r.status, body: r.body.toString() };
  }

  // ---- the forge proxy (TODO(slice A)) ---------------------------------------------------------

  const recordOf = (host, owner, name) => recorded.get(keyOf(host, owner, name)) ?? null;
  const refuse = (reason) => ({ verdict: 'refuse', reason });

  /** Judge one request. `rest` is the path after the repo. A short list: ADR 0128 D3d to D3f has the whole one. */
  async function judge(forge, host, method, pathname, query, body) {
    const segs = segmentsOf(pathname);
    if (!segs) return refuse('a path that is not canonical');
    for (const k of ['private_token', 'access_token', 'job_token', 'token']) if (query.has(k)) return refuse('a credential in the URL');
    const fromRepo = (owner, name, restSegs, restStr) => {
      const rec = recordOf(host, owner, name);
      if (!rec) return { verdict: 'refuse', reason: `${owner}/${name} is not one of this chat's repositories` };
      return { rec, rest: restSegs, restStr };
    };

    // git smart HTTP: /<owner>/<name>.git/<service path>
    if (segs.length >= 3 && segs[1].endsWith('.git')) {
      const r = fromRepo(segs[0], segs[1].slice(0, -4), segs.slice(2), `/${segs.slice(2).join('/')}`);
      if (r.verdict) return r;
      const tail = r.restStr;
      if (method === 'GET' && tail === '/info/refs') return { verdict: 'allow', git: true, rec: r.rec };
      if (method === 'POST' && tail === '/git-upload-pack') return { verdict: 'allow', git: true, rec: r.rec };
      if (method === 'POST' && tail === '/git-receive-pack') {
        const { cmds, options, unreadable } = pushCommands(body);
        if (unreadable) return refuse('a push this stand-in cannot read');
        if (options) return refuse('push options');
        for (const c of cmds) {
          if (!c.ref.startsWith('refs/heads/')) return refuse(`a push to ${c.ref}: only branches`);
          const branch = c.ref.slice('refs/heads/'.length);
          if (DEFAULT_BRANCHES.has(branch)) return refuse(`a push to the default branch ${branch}`);
          if (c.new === ZERO) return refuse(`deleting ${branch}`);
          const mine = created.has(`${keyOf(host, r.rec.owner, r.rec.name)}#${branch}`);
          if (c.old !== ZERO && !mine) return refuse(`${branch} is not a branch this chat created`);
        }
        // TODO(slice A): a non-fast-forward, a CI file (held, D8.3), and a branch name that triggers a deploy.
        return { verdict: 'allow', git: true, rec: r.rec, pushed: cmds.filter((c) => c.old === ZERO).map((c) => c.ref.slice('refs/heads/'.length)) };
      }
      return refuse('a git route that is not on the list');
    }

    if (forge.kind === 'github') {
      if (method === 'GET' && pathname === '/user') return { verdict: 'allow' };
      if (method === 'POST' && pathname === '/graphql') {
        let q;
        try {
          q = JSON.parse(body.toString());
        } catch {
          return refuse('a GraphQL body that is not JSON');
        }
        const d = describe(q.query ?? '', q.variables ?? {});
        if (d.roots.some((x) => ['node', 'nodes', 'search'].includes(x))) return refuse('a GraphQL root that reads any object');
        for (const full of d.repos) {
          const [o, n] = full.split('/');
          if (!recordOf(host, o, n)) return refuse(`${full} is not one of this chat's repositories`);
        }
        if (d.operation === 'mutation') {
          for (const m of d.mutationInputs) {
            if (MERGE_ROOTS.has(m.field)) {
              // TODO(slice F): resolve the node id through the forge; the fake's ids are `PR_<number>`.
              const number = Number(String(m.input?.pullRequestId ?? '').replace('PR_', ''));
              const rec = [...recorded.values()].find((x) => x.host === host);
              return { verdict: 'hold', rec, number, via: 'graphql mergePullRequest' };
            }
            if (!ALLOWED_MUTATIONS.has(m.field)) return refuse(`the GraphQL mutation ${m.field}`);
            if (m.field === 'createPullRequest' && ![...recorded.values()].some((x) => x.host === host && `R_${x.id}` === m.input?.repositoryId)) return refuse('a pull request in a repository that is not recorded');
          }
          return { verdict: 'allow' };
        }
        if (d.repos.length === 0 && !d.roots.every((x) => x === 'viewer')) return refuse('a GraphQL query that names no recorded repository');
        return { verdict: 'allow' };
      }
      if (segs[0] === 'repos' && segs.length >= 3) {
        const r = fromRepo(segs[1], segs[2], segs.slice(3), `/${segs.slice(3).join('/')}`);
        if (r.verdict) return r;
        if (method === 'GET') return { verdict: 'allow' };
        if (method === 'POST' && r.restStr === '/pulls') return { verdict: 'allow' };
        if (method === 'POST' && /^\/issues\/\d+\/comments$/.test(r.restStr)) return { verdict: 'allow' };
        const m = /^\/pulls\/(\d+)\/merge$/.exec(r.restStr);
        if (method === 'PUT' && m) return { verdict: 'hold', rec: r.rec, number: Number(m[1]), via: 'PUT merge' };
        return refuse(`${method} ${r.restStr} under a repository`);
      }
      return refuse(`${method} ${pathname}`);
    }

    // GitLab
    if (method === 'GET' && (pathname === '/api/v4/user' || pathname === '/api/v4/version')) return { verdict: 'allow' };
    if (segs[0] === 'api' && segs[1] === 'v4' && segs[2] === 'projects' && segs.length >= 4) {
      const [owner, ...nameParts] = segs[3].includes('/') ? segs[3].split('/') : [null];
      let rec = null;
      if (owner) rec = recordOf(host, owner, nameParts.join('/'));
      else rec = [...recorded.values()].find((x) => x.host === host && String(x.id) === segs[3]) ?? null;
      if (!rec) return refuse(`${segs[3]} is not one of this chat's repositories`);
      const restStr = `/${segs.slice(4).join('/')}`;
      if (method === 'GET') return { verdict: 'allow' };
      if (method === 'POST' && restStr === '/merge_requests') return { verdict: 'allow' };
      if (method === 'POST' && /^\/merge_requests\/\d+\/notes$/.test(restStr)) return { verdict: 'allow' };
      const m = /^\/merge_requests\/(\d+)\/merge$/.exec(restStr);
      if (method === 'PUT' && m) return { verdict: 'hold', rec, number: Number(m[1]), via: 'PUT merge' };
      return refuse(`${method} ${restStr} under a project`);
    }
    return refuse(`${method} ${pathname}`);
  }

  async function handle(req, res, host, via) {
    const forge = byHost.get(host);
    const send = (status, body, headers = {}) => {
      res.writeHead(status, { 'content-type': 'application/json', connection: 'close', ...headers });
      res.end(typeof body === 'string' ? body : JSON.stringify(body));
    };
    const record = (verdict, reason, extra = {}) => audit.push({ via, host, method: req.method, url: req.url.split('?')[0], verdict, reason, ...extra });
    if (!forge) {
      record('refuse', 'a host that is not a recorded forge');
      return send(403, { message: REFUSED });
    }
    const body = await readBody(req);
    const url = new URL(req.url, 'http://x');
    const creds = presented(req.headers);
    const credentialed = creds.length > 0;
    if (credentialed && !creds.every((c) => sameSecret(c, sentinel))) {
      record('refuse', 'a credential that is not this chat\'s sentinel');
      return send(403, { message: REFUSED });
    }
    // No credential at all is the anonymous route (D3i): git's first, unauthenticated request of a clone, forwarded with none.
    const isGitRead = /^\/[^/]+\/[^/]+\.git\/(info\/refs|git-upload-pack)$/.test(url.pathname) && !url.searchParams.get('service')?.includes('receive');
    if (!credentialed && !isGitRead) {
      // A push's first request carries no credential; git needs a 401 to ask its helper for one.
      const challenge = /^\/[^/]+\/[^/]+\.git\/info\/refs$/.test(url.pathname) && url.searchParams.get('service')?.includes('receive');
      record('refuse', challenge ? 'no sentinel yet: challenged' : 'no sentinel');
      return challenge ? send(401, { message: 'sign in' }, { 'www-authenticate': 'Basic realm="purlis"' }) : send(403, { message: REFUSED });
    }
    const forward = async (git, rec) => {
      const headers = {};
      for (const [k, v] of Object.entries(req.headers)) if (!HOP.has(k) && !STRIPPED.has(k)) headers[k] = v;
      const reqBody = Buffer.from(body);
      const call = async () => {
        const h = { ...headers, host };
        if (credentialed) {
          const tok = await accessFor(forge);
          h.authorization = git ? `Basic ${Buffer.from(`x-access-token:${tok}`).toString('base64')}` : `Bearer ${tok}`;
        }
        return upstream(forge, { method: req.method, pathname: url.pathname + url.search, headers: h, body: reqBody.length ? reqBody : null });
      };
      let r = await call();
      if (r.status === 401 && credentialed && keyring.get(forge.host)?.refresh) {
        await renew(forge, 'after a 401');
        r = await call();
      }
      const out = {};
      for (const [k, v] of Object.entries(r.headers)) if (!HOP.has(k)) out[k] = v;
      res.writeHead(r.status, { ...out, connection: 'close' });
      res.end(r.body);
      return rec;
    };

    if (!credentialed) {
      record('allow', 'anonymous git read');
      return forward(true, null);
    }
    const j = await judge(forge, host, req.method, url.pathname, url.searchParams, body);
    if (j.verdict === 'refuse') {
      record('refuse', j.reason);
      return send(403, { message: `${REFUSED} (${j.reason})` });
    }
    if (j.verdict === 'hold') {
      await raiseMerge(forge, j.rec, j.number, j.via);
      record('hold', j.via);
      return send(403, { message: HELD });
    }
    if (j.pushed) for (const b of j.pushed) created.add(`${keyOf(host, j.rec.owner, j.rec.name)}#${b}`);
    record('allow', j.git ? 'git' : 'api');
    return forward(Boolean(j.git), j.rec);
  }

  // ---- the listeners ----------------------------------------------------------------------------

  const servers = [];
  const listen = (server, ...args) =>
    new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(...args, () => {
        server.off('error', reject);
        servers.push(server);
        resolve(server.address());
      });
    });

  // gh: plain HTTP on the chat's unix socket; `github.localhost` is the one recorded GitHub host. TODO(slice A): two GitHub hosts.
  const githubHost = forges.github.host;
  await listen(http.createServer((req, res) => handle(req, res, githubHost, 'unix socket').catch((e) => { res.writeHead(502); res.end(String(e)); })), sockPath);

  // git and glab: a CONNECT the proxy terminates under the CA, for a recorded forge host only.
  const inside = http.createServer((req, res) => handle(req, res, req.socket.tunnelTo.split(':')[0], 'connect').catch((e) => { res.writeHead(502); res.end(String(e)); }));
  const connect = http.createServer((_, res) => {
    res.writeHead(405);
    res.end();
  });
  connect.on('connect', (req, client, head) => {
    const [host] = req.url.split(':');
    client.on('error', () => {});
    if (!byHost.has(host)) {
      audit.push({ via: 'connect', host, method: 'CONNECT', url: req.url, verdict: 'refuse', reason: 'a host that is not a recorded forge' });
      client.end('HTTP/1.1 403 Forbidden\r\nconnection: close\r\n\r\n');
      return;
    }
    client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
    const t = new tls.TLSSocket(client, { isServer: true, secureContext: tls.createSecureContext(ca.leafFor([host])) });
    t.tunnelTo = req.url;
    t.on('error', () => {});
    if (head?.length) client.unshift(head);
    inside.emit('connection', t);
  });
  const connectPort = (await listen(connect, 0, '127.0.0.1')).port;

  // ---- the chat's wiring (TODO(slice B)) --------------------------------------------------------

  /** A chat's folder, its environment, and a way to run a program in it. It holds the sentinel and no token. */
  function makeChat() {
    const dir = fs.mkdtempSync(path.join(work, 'chat-'));
    const home = path.join(dir, 'home');
    const bin = path.join(dir, 'bin');
    const projects = path.join(dir, 'projects');
    for (const d of [home, bin, projects, path.join(home, 'gh'), path.join(home, 'glab')]) fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(home, 'gh', 'config.yml'), `http_unix_socket: ${sockPath}\n`);
    fs.copyFileSync(ca.pem, path.join(dir, 'ca.pem'));
    const caFile = path.join(dir, 'ca.pem');

    const helper = path.join(bin, 'purlis-credential-helper');
    fs.writeFileSync(helper, `#!/bin/sh\n[ "$1" = get ] || exit 0\nprintf 'username=x-access-token\\npassword=%s\\n' "$PURLIS_FORGE_SENTINEL"\n`, { mode: 0o755 });

    // The gh wrapper (D3c): GH_HOST=github.localhost needs `-R github.localhost/<owner>/<repo>`, taken from the clone's remote.
    const realGh = tool('sh', ['-c', 'command -v gh']).trim();
    const wrapper = path.join(bin, 'gh');
    fs.writeFileSync(
      wrapper,
      `#!/bin/sh
# TODO(slice B): purlis writes this wrapper. It reads no output of gh.
REAL='${realGh}'
case "$1" in
  auth) [ "$2" = token ] && { echo "purlis: a chat has no token to print." >&2; exit 1; } ;;
  pr|issue|run|release|label)
    for a in "$@"; do case "$a" in -R|--repo|-R=*|--repo=*) exec "$REAL" "$@" ;; esac; done
    url=$(git config --get remote.origin.url 2>/dev/null)
    slug=$(printf '%s' "$url" | sed -e 's#^https://[^/]*/##' -e 's#\\.git$##')
    if [ -n "$slug" ] && [ "$2" != "" ]; then
      a=$1; b=$2; shift 2
      exec "$REAL" "$a" "$b" -R "github.localhost/$slug" "$@"
    fi ;;
esac
exec "$REAL" "$@"
`,
      { mode: 0o755 },
    );

    const gitKeys = [];
    for (const host of byHost.keys()) {
      gitKeys.push([`credential.https://${host}.helper`, ''], [`credential.https://${host}.helper`, helper], [`http.https://${host}/.sslCAInfo`, caFile]);
    }
    const author = ['chat', 'forge.invalid'].join('@');
    gitKeys.push(['user.name', 'chat'], ['user.email', author]);
    const env = {
      PATH: `${bin}${path.delimiter}${process.env.PATH}`,
      HOME: home,
      LANG: 'C',
      NO_COLOR: '1',
      TMPDIR: process.env.TMPDIR ?? '/tmp',
      PURLIS_FORGE_SENTINEL: sentinel,
      HTTPS_PROXY: `http://127.0.0.1:${connectPort}`,
      GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null',
      GIT_CONFIG_NOSYSTEM: '1',
      GIT_TERMINAL_PROMPT: '0',
      GIT_CONFIG_COUNT: String(gitKeys.length),
      ...Object.fromEntries(gitKeys.flatMap(([k, v], i) => [[`GIT_CONFIG_KEY_${i}`, k], [`GIT_CONFIG_VALUE_${i}`, v]])),
      GH_CONFIG_DIR: path.join(home, 'gh'),
      GH_HOST: 'github.localhost',
      GH_TOKEN: sentinel,
      GH_ENTERPRISE_TOKEN: sentinel,
      GH_NO_UPDATE_NOTIFIER: '1',
      GH_NO_EXTENSION_UPDATE_NOTIFIER: '1',
      GH_PROMPT_DISABLED: '1',
      GH_SPINNER_DISABLED: '1',
      GLAB_CONFIG_DIR: path.join(home, 'glab'),
      GITLAB_TOKEN: sentinel,
      GLAB_CA_CERT: caFile,
      GLAB_CHECK_UPDATE: 'false',
      GLAB_NO_PROMPT: '1',
      GLAB_SEND_TELEMETRY: 'false',
      NO_PROMPT: '1',
    };
    return {
      dir,
      home,
      projects,
      env,
      /** Run a program as the chat does. */
      exec: (cmd, args, cwd = projects) => run(cmd, args, { env, cwd }),
    };
  }

  return {
    sentinel,
    keyring,
    recorded,
    asks,
    audit,
    refreshes,
    connectPort,
    sockPath,
    signIn,
    asPerson,
    recordProject,
    approve,
    makeChat,
    /** The sign-in's PKCE variant, against the forge's authorize endpoint (a second sign-in; not kept). */
    async signInWithPkce(forge) {
      const verifier = crypto.randomBytes(32).toString('base64url');
      const challenge = crypto.createHash('sha256').update(verifier).digest('base64url');
      const authorize = forge.kind === 'github' ? '/login/oauth/authorize' : '/oauth/authorize';
      const q = new URLSearchParams({ client_id: CLIENT_ID, redirect_uri: 'http://127.0.0.1:1/cb', response_type: 'code', state: 's1', code_challenge: challenge, code_challenge_method: 'S256' });
      const a = await upstream(forge, { pathname: `${authorize}?${q}` });
      const code = new URL(a.headers.location).searchParams.get('code');
      const bad = await upstream(forge, { method: 'POST', pathname: paths(forge).token, ...form({ client_id: CLIENT_ID, grant_type: 'authorization_code', code, code_verifier: 'wrong' }) });
      const ok = await upstream(forge, { method: 'POST', pathname: paths(forge).token, ...form({ client_id: CLIENT_ID, grant_type: 'authorization_code', code, code_verifier: verifier }) });
      return { wrongVerifierRefused: Boolean(JSON.parse(bad.body).error), token: JSON.parse(ok.body).access_token };
    },
    close: () => servers.forEach((s) => s.close()),
  };
}
