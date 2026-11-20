#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **One login, slice E: how do `gh`, `glab` and `git` reach the forge proxy?** (a spike)
//
//   node tests/one-login-spike/spike.mjs [--out <dir>]
//
// A chat holds a sentinel and no token. purlis's forge proxy swaps the sentinel for the real
// credential on the way out. The design could not prove, inside a sandbox, HOW each client sends
// its calls to that proxy on Linux and macOS. This runs on a CI runner of each (the
// `one-login-proxy-spike` workflow) and tries each mechanism end to end:
//
//   chat's client --(sentinel)--> stand-in proxy --(real token)--> fake forge
//
// - The fake forge answers a GitHub-shaped and a GitLab-shaped API and git's smart-HTTP
//   advertisement, and says a request is good only when it carries the REAL token.
// - The stand-in proxy listens four ways (TLS, plain HTTP, a unix socket, and a CONNECT proxy that
//   terminates TLS under a CA of its own), swaps the sentinel and forwards.
// - Each case is one real client run with a scrubbed environment. A case "works" when the client
//   exits 0, prints what the forge answered, and the forge saw the REAL token and never the
//   sentinel.
//
// It reports and exits 0 whatever works: what works is the finding. It exits 1 only when the
// harness itself could not run. The fake forge of T1.1-50 (#7221, `tests/scenarios/src/net/forge.rs`)
// is not in `main` yet, so this carries a small one of its own with the same shape.

import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import tls from 'node:tls';
import zlib from 'node:zlib';

const SENTINEL = 'purlis-sentinel-7d3e9c';
const REAL = 'real-token-never-shown-to-the-chat-4c11';
const IS_CI = Boolean(process.env.CI);
const PLATFORM = process.platform === 'darwin' ? 'macOS' : process.platform === 'linux' ? 'Linux' : process.platform;

const outDir = (() => {
  const i = process.argv.indexOf('--out');
  return i > 0 ? path.resolve(process.argv[i + 1]) : fs.mkdtempSync(path.join(os.tmpdir(), 'one-login-out-'));
})();
fs.mkdirSync(outDir, { recursive: true });
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'one-login-spike-'));

const tool = (cmd, args, opts = {}) =>
  execFileSync(cmd, args, { stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8', timeout: 60_000, ...opts });

// ---- a CA of the spike's own, and a leaf per name set -----------------------------------------

const ca = { key: path.join(work, 'ca.key'), pem: path.join(work, 'ca.pem') };
tool('openssl', ['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', ca.key]);
fs.writeFileSync(
  path.join(work, 'ca.cnf'),
  '[req]\ndistinguished_name=dn\nx509_extensions=v3\nprompt=no\n[dn]\nCN=purlis spike CA\n' +
    '[v3]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\nsubjectKeyIdentifier=hash\n',
);
tool('openssl', ['req', '-x509', '-new', '-key', ca.key, '-config', path.join(work, 'ca.cnf'), '-days', '2', '-out', ca.pem]);
const caPem = fs.readFileSync(ca.pem, 'utf8');
const leafKey = path.join(work, 'leaf.key');
tool('openssl', ['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', leafKey]);

const leaves = new Map();
function leafFor(names) {
  const id = names.join(',');
  if (leaves.has(id)) return leaves.get(id);
  const stem = path.join(work, `leaf-${leaves.size}`);
  const san = names.map((n) => (net.isIP(n) ? `IP:${n}` : `DNS:${n}`)).join(',');
  fs.writeFileSync(
    `${stem}.ext`,
    `subjectAltName=${san}\nextendedKeyUsage=serverAuth\nkeyUsage=digitalSignature\nbasicConstraints=CA:FALSE\n`,
  );
  tool('openssl', ['req', '-new', '-key', leafKey, '-subj', `/CN=${names[0]}`, '-out', `${stem}.csr`]);
  tool('openssl', ['x509', '-req', '-in', `${stem}.csr`, '-CA', ca.pem, '-CAkey', ca.key, '-CAcreateserial', '-days', '2', '-extfile', `${stem}.ext`, '-out', `${stem}.pem`]);
  const leaf = { key: fs.readFileSync(leafKey), cert: fs.readFileSync(`${stem}.pem`) };
  leaves.set(id, leaf);
  return leaf;
}

// ---- a bare repository the fake forge advertises ----------------------------------------------

const bare = path.join(work, 'forge-repos', 'o', 'r.git');
{
  const src = path.join(work, 'src');
  const git = (cwd, ...a) =>
    tool('git', ['-c', 'user.name=spike', '-c', 'user.email=spike@example.invalid', ...a], { cwd, env: { ...process.env, GIT_CONFIG_GLOBAL: os.devNull, GIT_CONFIG_NOSYSTEM: '1' } });
  fs.mkdirSync(src);
  git(src, 'init', '-q', '-b', 'main');
  fs.mkdirSync(path.join(src, 'src'));
  fs.writeFileSync(path.join(src, 'Cargo.toml'), '[package]\nname = "r"\nversion = "0.1.0"\nedition = "2021"\n');
  fs.writeFileSync(path.join(src, 'src', 'lib.rs'), '');
  fs.writeFileSync(path.join(src, 'package.json'), '{"name":"r","version":"0.1.0"}\n');
  fs.writeFileSync(path.join(src, 'setup.py'), "from setuptools import setup\nsetup(name='r', version='0.1')\n");
  git(src, 'add', '-A');
  git(src, 'commit', '-q', '-m', 'spike');
  fs.mkdirSync(path.dirname(bare), { recursive: true });
  git(work, 'clone', '-q', '--bare', src, bare);
}

// ---- the fake forge ---------------------------------------------------------------------------

/** Every request the forge got: what a case asserts on. */
const forgeLog = [];

function credentialOf(headers) {
  const auth = headers.authorization ?? '';
  if (/^basic /i.test(auth)) {
    const decoded = Buffer.from(auth.slice(6), 'base64').toString();
    return decoded.slice(decoded.indexOf(':') + 1);
  }
  if (auth) return auth.replace(/^(token|bearer) /i, '');
  return headers['private-token'] ?? null;
}

const pkt = (s) => (s.length + 4).toString(16).padStart(4, '0') + s;

function forgeHandler(req, res) {
  const credential = credentialOf(req.headers);
  const ok = credential === REAL;
  const normal = req.url
    .replace(/^\/api\/v3(?=\/|\?|$)/, '')
    .replace(/^\/api\/graphql(?=\?|$)/, '/graphql')
    .replace(/^\/o\/r(?=\/(info\/refs|git-upload-pack))/, '/o/r.git'); // cargo asks for the URL without .git
  const entry = {
    method: req.method,
    path: req.url,
    host: req.headers.host,
    ok,
    sawSentinel: String(credential ?? '').includes(SENTINEL) || JSON.stringify(req.headers).includes(SENTINEL),
    status: 0,
  };
  forgeLog.push(entry);
  const json = (status, body) => {
    entry.status = status;
    res.writeHead(status, { 'content-type': 'application/json', 'x-forge': 'fake', connection: 'close' });
    res.end(JSON.stringify(body));
  };
  const chunks = [];
  req.on('data', (c) => chunks.push(c));
  req.on('end', () => {
    if (!ok) {
      entry.status = 401;
      res.writeHead(401, { 'content-type': 'application/json', 'www-authenticate': 'Basic realm="forge"', connection: 'close' });
      res.end(JSON.stringify({ message: 'Bad credentials' }));
      return;
    }
    const host = req.headers.host;
    const body = Buffer.concat(chunks).toString();
    if (body) entry.body = body.slice(0, 700);
    const method = req.method === 'HEAD' ? 'GET' : req.method;
    if (method === 'GET' && normal.startsWith('/o/r.git/info/refs')) {
      const out = [];
      const child = spawn('git', ['upload-pack', '--stateless-rpc', '--advertise-refs', bare]);
      child.stdout.on('data', (c) => out.push(c));
      child.on('close', () => {
        entry.status = 200;
        res.writeHead(200, { 'content-type': 'application/x-git-upload-pack-advertisement', 'cache-control': 'no-cache', connection: 'close' });
        res.end(Buffer.concat([Buffer.from(pkt('# service=git-upload-pack\n') + '0000'), ...out]));
      });
      return;
    }
    if (method === 'GET' && /^\/o\/r\/(legacy\.)?(tar\.gz|tarball)\//.test(normal)) {
      const out = [];
      const child = spawn('git', ['-C', bare, 'archive', '--format=tar.gz', '--prefix=r-main/', 'main']);
      child.stdout.on('data', (c) => out.push(c));
      child.on('close', () => {
        entry.status = 200;
        res.writeHead(200, { 'content-type': 'application/x-gzip', connection: 'close' });
        res.end(Buffer.concat(out));
      });
      return;
    }
    if (method === 'POST' && normal.startsWith('/o/r.git/git-upload-pack')) {
      const out = [];
      const child = spawn('git', ['upload-pack', '--stateless-rpc', bare]);
      child.stdout.on('data', (c) => out.push(c));
      child.on('close', () => {
        entry.status = 200;
        res.writeHead(200, { 'content-type': 'application/x-git-upload-pack-result', 'cache-control': 'no-cache', connection: 'close' });
        res.end(Buffer.concat(out));
      });
      const raw = Buffer.concat(chunks);
      child.stdin.end(req.headers['content-encoding'] === 'gzip' ? zlib.gunzipSync(raw) : raw);
      return;
    }
    if (method === 'GET' && normal.split('?')[0] === '/') return json(200, {});
    if (method === 'GET' && normal.split('?')[0] === '/user') return json(200, { login: 'fake-user', id: 1 });
    if (method === 'GET' && normal.split('?')[0] === '/api/v4/user') return json(200, { username: 'fake-user', id: 1 });
    if (method === 'GET' && normal.split('?')[0] === '/repos/o/r')
      return json(200, { full_name: 'o/r', name: 'r', default_branch: 'main', html_url: `https://${host}/o/r`, private: false });
    if (method === 'GET' && normal.split('?')[0] === '/api/v4/projects/o%2Fr')
      return json(200, { id: 42, name: 'r', path: 'r', path_with_namespace: 'o/r', default_branch: 'main', name_with_namespace: 'o / r', description: 'fake', web_url: `http://${host}/o/r`, http_url_to_repo: `http://${host}/o/r.git`, visibility: 'private' });
    if (method === 'POST' && normal.split('?')[0] === '/graphql') {
      if (body.includes('viewer')) return json(200, { data: { viewer: { login: 'fake-user' } } });
      const repository = { databaseId: 1, sshUrl: 'git@github.com:o/r.git', hasIssuesEnabled: true, hasWikiEnabled: false, viewerPermission: 'ADMIN', mergeCommitAllowed: true, rebaseMergeAllowed: true, squashMergeAllowed: true, parent: null, id: 'R_1', name: 'r', nameWithOwner: 'o/r', url: `https://${host}/o/r`, description: 'fake', isPrivate: false, defaultBranchRef: { name: 'main' }, owner: { login: 'o' } };
      const aliases = [...body.matchAll(/(\w+)\s*:\s*repository\s*\(/g)].map((m) => m[1]);
      return json(200, { data: { repository, ...Object.fromEntries(aliases.map((a) => [a, repository])) } });
    }
    return json(404, { message: `fake forge: no answer for ${req.method} ${req.url}` });
  });
}

// ---- the stand-in proxy -----------------------------------------------------------------------

/** Swap the sentinel for the real token in whatever header carries it. Basic is decoded first. */
function swapped(headers) {
  const out = { ...headers };
  let did = false;
  for (const k of ['authorization', 'private-token']) {
    const v = out[k];
    if (typeof v !== 'string') continue;
    if (v.includes(SENTINEL)) {
      out[k] = v.replaceAll(SENTINEL, REAL);
      did = true;
    } else if (/^basic /i.test(v)) {
      const decoded = Buffer.from(v.slice(6), 'base64').toString();
      if (decoded.includes(SENTINEL)) {
        out[k] = `Basic ${Buffer.from(decoded.replaceAll(SENTINEL, REAL)).toString('base64')}`;
        did = true;
      }
    }
  }
  return { headers: out, did };
}

/** What each proxy listener saw: how the client arrived, and the Host it asked for. */
const proxyLog = [];

function proxyHandler(via) {
  return (req, res) => {
    const arrived = typeof via === 'function' ? via(req) : via;
    // A client that treats this as its HTTP proxy sends the absolute form (`GET http://host/path`).
    const target = /^https?:\/\//i.test(req.url) ? new URL(req.url) : null;
    const asked = target ? target.pathname + target.search : req.url;
    const { headers, did } = swapped(req.headers);
    delete headers.connection;
    delete headers['proxy-connection'];
    proxyLog.push({ via: target ? `${arrived} (absolute-form)` : arrived, method: req.method, url: req.url, host: req.headers.host, swapped: did });
    const up = https.request(
      { host: '127.0.0.1', port: ports.forge, method: req.method, path: asked, headers, ca: caPem, servername: 'localhost' },
      (r) => {
        res.writeHead(r.statusCode, r.headers);
        r.pipe(res);
      },
    );
    up.on('error', (e) => {
      res.writeHead(502);
      res.end(`proxy: ${e.message}`);
    });
    req.pipe(up);
  };
}

const ports = {};
const sockPath = path.join(work, 'proxy.sock');
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

const BASIC_NAMES = ['127.0.0.1', 'localhost', 'github.localhost', 'api.github.localhost'];

async function startServers() {
  const forgeLeaf = leafFor(['127.0.0.1', 'localhost']);
  ports.forge = (await listen(https.createServer(forgeLeaf, forgeHandler), 0, '127.0.0.1')).port;
  // The proxy, reached directly: TLS on a loopback port, plain HTTP on one, plain HTTP on a unix socket.
  ports.tls = (await listen(https.createServer(leafFor(BASIC_NAMES), proxyHandler('tls')), 0, '127.0.0.1')).port;
  ports.http = (await listen(http.createServer(proxyHandler('http')), 0, '127.0.0.1')).port;
  await listen(http.createServer(proxyHandler('unix')), sockPath);
  // Port 80, for `gh`'s http://api.github.localhost/: only where the runner lets this user bind it.
  try {
    await listen(http.createServer(proxyHandler('port80')), 80, '127.0.0.1');
    ports.eighty = 80;
  } catch (e) {
    ports.eighty = null;
    ports.eightyWhy = e.code ?? String(e);
  }
  // The proxy, reached as a CONNECT proxy (HTTPS_PROXY / http.proxy): it terminates TLS for the
  // host the client asked to tunnel to, under the spike's CA, and serves what is inside.
  const inside = http.createServer(proxyHandler((req) => `connect:${req.socket.tunnelTo}`));
  const connect = http.createServer((_, res) => {
    res.writeHead(405);
    res.end();
  });
  connect.on('connect', (req, client, head) => {
    const [host] = req.url.split(':');
    client.on('error', () => {});
    client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
    const leaf = leafFor([host]);
    const t = new tls.TLSSocket(client, { isServer: true, secureContext: tls.createSecureContext(leaf) });
    t.tunnelTo = req.url;
    t.on('error', () => {});
    if (head?.length) client.unshift(head);
    inside.emit('connection', t);
  });
  ports.connect = (await listen(connect, 0, '127.0.0.1')).port;
}

// ---- running a client -------------------------------------------------------------------------

function exec(cmd, args, { env, cwd, timeout = 20_000 }) {
  return new Promise((resolve) => {
    let stdout = '';
    let stderr = '';
    let child;
    try {
      child = spawn(cmd, args, { env, cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (e) {
      resolve({ code: null, stdout, stderr: String(e) });
      return;
    }
    const timer = setTimeout(() => child.kill('SIGKILL'), timeout);
    child.stdout.on('data', (c) => (stdout += c));
    child.stderr.on('data', (c) => (stderr += c));
    child.on('error', (e) => {
      clearTimeout(timer);
      resolve({ code: null, stdout, stderr: stderr + String(e) });
    });
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      resolve({ code: signal ? `signal ${signal}` : code, stdout, stderr });
    });
  });
}

const versionOf = (cmd, args) => {
  try {
    return tool(cmd, args).split('\n')[0].trim();
  } catch {
    return 'not installed';
  }
};

// ---- the cases --------------------------------------------------------------------------------

const GH_QUIET = { GH_NO_UPDATE_NOTIFIER: '1', GH_NO_EXTENSION_UPDATE_NOTIFIER: '1', GH_PROMPT_DISABLED: '1', GH_SPINNER_DISABLED: '1' };
const GLAB_QUIET = { GLAB_CHECK_UPDATE: 'false', GLAB_NO_PROMPT: '1', GLAB_SEND_TELEMETRY: 'false' };

/** `trust` is how a client is told to trust the spike's CA: `env` (the client's own variable or setting) or `os` (the platform's store). */
function cases() {
  const gh = (home, extra = {}) => ({ GH_CONFIG_DIR: path.join(home, 'gh'), ...GH_QUIET, ...extra });
  const glab = (home, extra = {}) => ({ GLAB_CONFIG_DIR: path.join(home, 'glab'), ...GLAB_QUIET, ...extra });
  const sslFile = (trust) => (trust === 'env' ? { SSL_CERT_FILE: ca.pem } : {});
  const list = [];
  const add = (c) => list.push(c);

  // gh ------------------------------------------------------------------------------------------
  const ghCommands = [
    ['api user', ['api', 'user'], 'fake-user'],
    ['api graphql', ['api', 'graphql', '-f', 'query={ viewer { login } }'], 'fake-user'],
    ['api repos/o/r', ['api', 'repos/o/r'], 'o/r'],
    ['repo view o/r', ['repo', 'view', 'o/r', '--json', 'name,url'], '"name":"r"', { info: true }],
    ['auth status', ['auth', 'status'], 'fake-user', { info: true }],
  ];
  for (const [label, args, expect, extra] of ghCommands) {
    add({
      tool: 'gh',
      mechanism: 'GH_HOST=github.localhost + http_unix_socket (plain http over a unix socket)',
      label,
      phases: ['env'],
      cmd: ['gh', ...args],
      expect,
      ...extra,
      setup(home) {
        fs.mkdirSync(path.join(home, 'gh'), { recursive: true });
        fs.writeFileSync(path.join(home, 'gh', 'config.yml'), `http_unix_socket: ${sockPath}\n`);
      },
      env: (home) => gh(home, { GH_HOST: 'github.localhost', GH_TOKEN: SENTINEL, GH_ENTERPRISE_TOKEN: SENTINEL }),
    });
  }
  add({
    tool: 'gh',
    mechanism: 'GH_HOST=github.localhost, http on port 80 (needs api.github.localhost to resolve, and port 80)',
    label: 'api user',
    phases: ['env'],
    needs: () => (ports.eighty ? null : `port 80 not bindable (${ports.eightyWhy})`),
    cmd: ['gh', 'api', 'user'],
    expect: 'fake-user',
    env: (home) => gh(home, { GH_HOST: 'github.localhost', GH_TOKEN: SENTINEL, GH_ENTERPRISE_TOKEN: SENTINEL }),
  });
  for (const [label, args, expect, extra] of ghCommands.slice(0, 3)) {
    add({
      tool: 'gh',
      mechanism: 'GH_HOST=127.0.0.1:<port> as an enterprise host, https, GH_ENTERPRISE_TOKEN',
      label,
      phases: ['env', 'os'],
      tls: true,
      cmd: ['gh', ...args],
      expect,
      ...extra,
      env: (home, trust) => gh(home, { GH_HOST: `127.0.0.1:${ports.tls}`, GH_ENTERPRISE_TOKEN: SENTINEL, ...sslFile(trust) }),
    });
  }
  for (const [label, args, expect, extra] of ghCommands) {
    add({
      tool: 'gh',
      mechanism: 'HTTPS_PROXY to a CONNECT proxy that terminates TLS under the spike CA (host stays github.com)',
      label,
      phases: ['env', 'os'],
      tls: true,
      cmd: ['gh', ...args],
      expect,
      ...extra,
      env: (home, trust) => gh(home, { GH_TOKEN: SENTINEL, HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, ...sslFile(trust) }),
    });
  }

  // glab ----------------------------------------------------------------------------------------
  const glabCommands = [
    ['api user', ['api', 'user'], 'fake-user'],
    ['api projects/o%2Fr', ['api', 'projects/o%2Fr'], 'o/r'],
    ['repo view o/r', ['repo', 'view', 'o/r'], 'o/r', { info: true }],
  ];
  for (const [label, args, expect, extra] of glabCommands) {
    add({
      tool: 'glab',
      mechanism: 'GITLAB_HOST=127.0.0.1:<port> + GLAB_API_PROTOCOL=http',
      label,
      phases: ['env'],
      cmd: ['glab', ...args],
      expect,
      ...extra,
      env: (home) => glab(home, { GITLAB_HOST: `127.0.0.1:${ports.http}`, GLAB_API_PROTOCOL: 'http', GITLAB_TOKEN: SENTINEL }),
    });
    add({
      tool: 'glab',
      mechanism: 'GITLAB_HOST=127.0.0.1:<port>, https, trust by GLAB_CA_CERT (env) or the OS store',
      label,
      phases: ['env', 'os'],
      tls: true,
      cmd: ['glab', ...args],
      expect,
      ...extra,
      env: (home, trust) => glab(home, { GITLAB_HOST: `127.0.0.1:${ports.tls}`, GITLAB_TOKEN: SENTINEL, ...(trust === 'env' ? { GLAB_CA_CERT: ca.pem } : {}) }),
    });
    add({
      tool: 'glab',
      mechanism: 'HTTPS_PROXY to a CONNECT proxy that terminates TLS under the spike CA (host stays gitlab.com), trust by GLAB_CA_CERT (env) or the OS store',
      label,
      phases: ['env', 'os'],
      tls: true,
      cmd: ['glab', ...args],
      expect,
      ...extra,
      env: (home, trust) => glab(home, { GITLAB_TOKEN: SENTINEL, HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, ...(trust === 'env' ? { GLAB_CA_CERT: ca.pem } : {}) }),
    });
  }

  // git -----------------------------------------------------------------------------------------
  const helper = path.join(work, 'credential-sentinel.sh');
  fs.writeFileSync(helper, `#!/bin/sh\n[ "$1" = get ] && printf 'username=x-access-token\\npassword=${SENTINEL}\\n'\nexit 0\n`, { mode: 0o755 });
  const gitBase = ['-c', 'protocol.version=0', '-c', 'credential.helper=', '-c', `credential.helper=${helper}`];
  const gitEnv = (home) => ({ GIT_CONFIG_GLOBAL: path.join(home, '.gitconfig'), GIT_CONFIG_NOSYSTEM: '1', GIT_TERMINAL_PROMPT: '0' });
  add({
    tool: 'git',
    mechanism: 'url.<proxy>.insteadOf + credential helper answering the sentinel (https, http.sslCAInfo)',
    label: 'ls-remote https://github.com/o/r.git',
    phases: ['env', 'os'],
    tls: true,
    cmd: (trust) => [
      'git',
      ...gitBase,
      '-c', `url.https://127.0.0.1:${ports.tls}/.insteadOf=https://github.com/`,
      ...(trust === 'env' ? ['-c', `http.sslCAInfo=${ca.pem}`] : []),
      'ls-remote', 'https://github.com/o/r.git',
    ],
    expect: 'refs/heads/main',
    env: (home) => gitEnv(home),
  });
  add({
    tool: 'git',
    mechanism: 'url.<proxy>.insteadOf with plain http to the proxy + credential helper (no TLS to trust)',
    label: 'ls-remote https://github.com/o/r.git',
    phases: ['env'],
    cmd: () => [
      'git',
      ...gitBase,
      '-c', `url.http://127.0.0.1:${ports.http}/.insteadOf=https://github.com/`,
      '-c', 'http.followRedirects=false',
      'ls-remote', 'https://github.com/o/r.git',
    ],
    expect: 'refs/heads/main',
    env: (home) => gitEnv(home),
  });
  add({
    tool: 'git',
    mechanism: 'http.<url>.proxy to a CONNECT proxy that terminates TLS under the spike CA (url stays github.com)',
    label: 'ls-remote https://github.com/o/r.git',
    phases: ['env', 'os'],
    tls: true,
    cmd: (trust) => [
      'git',
      ...gitBase,
      '-c', `http.https://github.com/.proxy=http://127.0.0.1:${ports.connect}`,
      ...(trust === 'env' ? ['-c', `http.https://github.com/.sslCAInfo=${ca.pem}`] : []),
      'ls-remote', 'https://github.com/o/r.git',
    ],
    expect: 'refs/heads/main',
    env: (home) => gitEnv(home),
  });
  // The chat's own environment (instruction 181): HTTP(S)_PROXY at the proxy, NO_PROXY=localhost.
  for (const [noProxy, note] of [['', 'NO_PROXY cleared'], ['localhost,127.0.0.1', 'NO_PROXY=localhost,127.0.0.1 as a chat has it']]) {
    add({
      tool: 'gh',
      mechanism: `GH_HOST=github.localhost + HTTP_PROXY at the proxy's http port (${note})`,
      label: 'api user',
      phases: ['env'],
      cmd: ['gh', 'api', 'user'],
      expect: 'fake-user',
      env: (home) => gh(home, { GH_HOST: 'github.localhost', GH_TOKEN: SENTINEL, HTTP_PROXY: `http://127.0.0.1:${ports.http}`, HTTPS_PROXY: `http://127.0.0.1:${ports.http}`, NO_PROXY: noProxy, no_proxy: noProxy }),
    });
  }
  const gitGlobal = (home) => {
    fs.writeFileSync(path.join(home, '.gitconfig'), `[credential]\n\thelper =\n\thelper = ${path.join(work, 'credential-sentinel.sh')}\n[protocol]\n\tversion = 0\n`);
  };
  const viaConnect = (home, trust) => ({ HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, ...(trust === 'env' ? { SSL_CERT_FILE: ca.pem, GIT_SSL_CAINFO: ca.pem } : {}), ...gitEnv(home) });
  add({
    tool: 'git',
    mechanism: 'HTTPS_PROXY env alone (no git config) to a CONNECT proxy that terminates TLS under the spike CA; GIT_SSL_CAINFO',
    label: 'ls-remote https://github.com/o/r.git',
    phases: ['env', 'os'],
    tls: true,
    setup: gitGlobal,
    cmd: ['git', 'ls-remote', 'https://github.com/o/r.git'],
    expect: 'refs/heads/main',
    env: (home, trust) => viaConnect(home, trust),
  });
  add({
    tool: 'git',
    mechanism: 'HTTPS_PROXY env alone: a whole clone, then what `git remote -v` records',
    label: 'clone https://github.com/o/r.git; remote -v',
    phases: ['env', 'os'],
    tls: true,
    info: true,
    setup: gitGlobal,
    cmd: ['sh', '-c', 'git clone -q https://github.com/o/r.git c && git -C c remote -v'],
    expect: 'https://github.com/o/r.git (fetch)',
    env: (home, trust) => viaConnect(home, trust),
  });
  add({
    tool: 'gh',
    mechanism: 'HTTPS_PROXY to the CONNECT proxy (host stays github.com): gh repo clone, then `git remote -v`',
    label: 'repo clone o/r; remote -v',
    phases: ['env', 'os'],
    tls: true,
    info: true,
    setup: gitGlobal,
    cmd: ['sh', '-c', 'gh repo clone o/r c && git -C c remote -v'],
    expect: 'github.com/o/r.git (fetch)',
    env: (home, trust) => gh(home, { GH_TOKEN: SENTINEL, ...viaConnect(home, trust) }),
  });
  add({
    tool: 'gh',
    mechanism: 'GH_HOST=127.0.0.1:<port> as an enterprise host: gh repo clone, then `git remote -v`',
    label: 'repo clone o/r; remote -v',
    phases: ['env', 'os'],
    tls: true,
    info: true,
    setup: gitGlobal,
    cmd: ['sh', '-c', 'gh repo clone o/r c && git -C c remote -v'],
    expect: '/o/r.git (fetch)',
    env: (home, trust) => gh(home, { GH_HOST: `127.0.0.1:${ports.tls}`, GH_ENTERPRISE_TOKEN: SENTINEL, ...(trust === 'env' ? { SSL_CERT_FILE: ca.pem, GIT_SSL_CAINFO: ca.pem } : {}), ...gitEnv(home) }),
  });
  add({
    tool: 'gh',
    mechanism: 'HTTPS_PROXY to the CONNECT proxy: gh repo clone by URL (no API lookup), then `git remote -v`',
    label: 'repo clone https://github.com/o/r; remote -v',
    phases: ['env'],
    tls: true,
    info: true,
    setup: gitGlobal,
    cmd: ['sh', '-c', 'gh repo clone https://github.com/o/r c && git -C c remote -v'],
    expect: 'github.com/o/r',
    env: (home, trust) => gh(home, { GH_TOKEN: SENTINEL, ...viaConnect(home, trust) }),
  });
  for (const [mech, envFor] of [
    ['HTTPS_PROXY to the CONNECT proxy (host stays github.com)', (home, trust) => gh(home, { GH_TOKEN: SENTINEL, HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, ...sslFile(trust) })],
    ['GH_HOST=127.0.0.1:<port> as an enterprise host', (home, trust) => gh(home, { GH_HOST: `127.0.0.1:${ports.tls}`, GH_ENTERPRISE_TOKEN: SENTINEL, ...sslFile(trust) })],
  ]) {
    add({
      tool: 'gh',
      mechanism: `${mech}: what gh browse prints`,
      label: 'browse --no-browser -R o/r',
      phases: ['env'],
      tls: true,
      info: true,
      cmd: ['gh', 'browse', '--no-browser', '-R', 'o/r'],
      expect: '/o/r',
      env: envFor,
    });
  }
  // gh's remote matching (instruction 184): GH_HOST=github.localhost while the clone's remote says
  // github.com, or github.localhost. A case "works" when a request REACHED the forge (the fake has
  // no pull request endpoints, so what gh does with the answer is not the question).
  const repoWithRemote = (url) => (home) => {
    gitGlobal(home);
    const r = path.join(home, 'repo');
    fs.mkdirSync(r);
    const g = (...a) => tool('git', ['-C', r, ...a], { env: { ...process.env, ...gitEnv(home) } });
    g('init', '-q', '-b', 'main');
    g('remote', 'add', 'origin', url);
    fs.mkdirSync(path.join(home, 'gh'), { recursive: true });
    fs.writeFileSync(path.join(home, 'gh', 'config.yml'), `http_unix_socket: ${sockPath}\n`);
  };
  const ghLocal = (home) => ({ ...gh(home, { GH_HOST: 'github.localhost', GH_TOKEN: SENTINEL, GH_ENTERPRISE_TOKEN: SENTINEL }), ...gitEnv(home) });
  const matching = [
    ['remote github.com, no -R', 'https://github.com/o/r.git', ['pr', 'view']],
    ['remote github.com, no -R', 'https://github.com/o/r.git', ['pr', 'checks']],
    ['remote github.localhost, no -R', 'https://github.localhost/o/r.git', ['pr', 'view']],
    ['remote github.localhost, no -R', 'https://github.localhost/o/r.git', ['pr', 'checks']],
    ['remote github.com, -R o/r', 'https://github.com/o/r.git', ['pr', 'view', '1', '-R', 'o/r']],
    ['remote github.com, -R github.localhost/o/r', 'https://github.com/o/r.git', ['pr', 'view', '1', '-R', 'github.localhost/o/r']],
    ['remote github.com, no -R', 'https://github.com/o/r.git', ['pr', 'create', '--title', 't', '--body', 'b', '--head', 'main', '--base', 'main']],
  ];
  for (const [label, url, args] of matching) {
    add({
      tool: 'gh',
      mechanism: `GH_HOST=github.localhost + unix socket; ${label}`,
      label: args.join(' '),
      phases: ['env'],
      reachOnly: true,
      info: true,
      setup: repoWithRemote(url),
      cmd: ['sh', '-c', `cd repo && gh ${args.map((a) => `'${a}'`).join(' ')}`],
      expect: '',
      env: ghLocal,
    });
  }

  // cargo's git dependency: libgit2, then the git CLI.
  const cargoApp = (home) => {
    gitGlobal(home);
    // libgit2 reads the helper list itself and does not know git's empty-entry reset: one helper, no reset.
    fs.writeFileSync(path.join(home, '.gitconfig'), `[credential]\n\thelper = ${path.join(work, 'credential-sentinel.sh')}\n[protocol]\n\tversion = 0\n`);
    fs.mkdirSync(path.join(home, 'app', 'src'), { recursive: true });
    fs.writeFileSync(path.join(home, 'app', 'Cargo.toml'), '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\nr = { git = "https://github.com/o/r" }\n');
    fs.writeFileSync(path.join(home, 'app', 'src', 'lib.rs'), '');
  };
  const rust = { RUSTUP_HOME: process.env.RUSTUP_HOME ?? path.join(os.homedir(), '.rustup'), CARGO_TERM_COLOR: 'never' };
  for (const [label, cli] of [['libgit2 (cargo built in)', 'false'], ['git CLI (net.git-fetch-with-cli)', 'true']]) {
    add({
      tool: 'cargo',
      mechanism: `git dependency through HTTPS_PROXY to the CONNECT proxy, ${label}; CARGO_HTTP_CAINFO + GIT_SSL_CAINFO`,
      label: 'fetch',
      phases: ['env'],
      tls: true,
      info: true,
      setup: cargoApp,
      cmd: ['sh', '-c', 'cd app && cargo fetch'],
      expect: '',
      reachOnly: true,
      env: (home) => ({ ...rust, ...gitEnv(home), CARGO_HOME: path.join(home, 'cargo'), CARGO_NET_GIT_FETCH_WITH_CLI: cli, HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, CARGO_HTTP_CAINFO: ca.pem, GIT_SSL_CAINFO: ca.pem, SSL_CERT_FILE: ca.pem }),
    });
  }

  // a GitHub tarball (codeload.github.com) through the CONNECT proxy.
  const tarball = 'https://codeload.github.com/o/r/tar.gz/refs/heads/main';
  add({
    tool: 'npm',
    mechanism: 'tarball URL through HTTPS_PROXY to the CONNECT proxy; NODE_EXTRA_CA_CERTS; .npmrc _authToken for codeload.github.com',
    label: 'install codeload tarball',
    phases: ['env'],
    tls: true,
    info: true,
    reachOnly: true,
    setup: (home) => {
      fs.mkdirSync(path.join(home, 'p'), { recursive: true });
      fs.writeFileSync(path.join(home, 'p', 'package.json'), '{"name":"p","version":"1.0.0"}\n');
      fs.writeFileSync(path.join(home, '.npmrc'), `//codeload.github.com/:_authToken=${SENTINEL}\n`);
    },
    cmd: ['sh', '-c', `cd p && npm install --no-audit --no-fund ${tarball}`],
    expect: '',
    env: () => ({ HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, NODE_EXTRA_CA_CERTS: ca.pem }),
  });
  add({
    tool: 'pip',
    mechanism: 'tarball URL with the sentinel as URL user info, through HTTPS_PROXY to the CONNECT proxy; PIP_CERT',
    label: 'download codeload tarball',
    phases: ['env'],
    tls: true,
    info: true,
    reachOnly: true,
    cmd: ['sh', '-c', `python3 -m pip download --no-deps --no-build-isolation -d out 'https://x-access-token:${SENTINEL}@codeload.github.com/o/r/tar.gz/refs/heads/main'`],
    expect: '',
    env: () => ({ HTTPS_PROXY: `http://127.0.0.1:${ports.connect}`, PIP_CERT: ca.pem }),
  });
  return list;
}

const results = [];
let flush = () => {};

async function runCase(c, phase) {
  const home = fs.mkdtempSync(path.join(work, 'home-'));
  const base = { PATH: process.env.PATH, HOME: home, LANG: 'C', NO_COLOR: '1', TMPDIR: process.env.TMPDIR ?? os.tmpdir() };
  const trust = phase;
  const extra = c.env(home, trust);
  const row = { os: PLATFORM, tool: c.tool, mechanism: c.mechanism, command: c.label, trust: c.tls ? (phase === 'env' ? 'client variable/setting' : 'OS trust store') : 'none needed', info: Boolean(c.info) };
  const skip = c.needs?.();
  if (skip) {
    results.push({ ...row, verdict: 'not run', detail: skip });
    return;
  }
  console.log(`start  [${trust}] ${c.tool}: ${c.mechanism} :: ${c.label}`);
  c.setup?.(home);
  const cmd = typeof c.cmd === 'function' ? c.cmd(trust) : c.cmd;
  const mark = forgeLog.length;
  const proxyMark = proxyLog.length;
  const r = await exec(cmd[0], cmd.slice(1), { env: { ...base, ...extra }, cwd: home });
  const seen = forgeLog.slice(mark);
  const proxied = proxyLog.slice(proxyMark);
  const good = seen.filter((e) => e.ok && e.status < 400);
  const leaked = (r.stdout + r.stderr).includes(REAL);
  const sentinelAtForge = seen.some((e) => e.sawSentinel);
  const works = c.reachOnly
    ? good.length > 0 && !sentinelAtForge && !leaked
    : r.code === 0 && r.stdout.includes(c.expect) && good.length > 0 && !sentinelAtForge && !leaked;
  const lastLine = (s) => s.trim().split('\n').filter(Boolean).slice(-2).join(' | ').slice(0, 300);
  console.log(`${works ? 'works' : 'FAILS'}  [${trust}] ${c.tool}: ${c.mechanism} :: ${c.label} (exit ${r.code})`);
  results.push({
    ...row,
    verdict: works ? 'works' : 'fails',
    detail: works
      ? `forge saw ${good.map((e) => `${e.method} ${e.path} (Host ${e.host})`).join('; ')}${c.reachOnly ? `; exit ${r.code}: ${(r.stderr || r.stdout).trim().split('\n').slice(0, 2).join(' | ').slice(0, 300)}` : ''}`
      : `exit ${r.code}; ${(r.stderr || r.stdout).trim().split('\n').slice(0, 2).join(' | ').slice(0, 300) || 'no output'}; the forge saw ${seen.length} request(s) [${seen.map((e) => `${e.method} ${e.path} -> ${e.status}${e.body && c.label.includes('clone') ? ` body ${e.body}` : ''}`).join('; ')}], the proxy ${proxied.length} [${proxied.map((e) => `${e.via} ${e.method} ${e.url}`).join('; ')}]`,
    ...(leaked ? { leaked: true } : {}),
    ...(sentinelAtForge ? { sentinelReachedForge: true } : {}),
    output: lastLine(r.stdout),
  });
  flush();
}

// ---- trusting the CA the way the OS does ------------------------------------------------------

const osTrustFile = '/usr/local/share/ca-certificates/purlis-spike-ca.crt';

function installOsTrust() {
  if (process.platform === 'darwin') {
    tool('sudo', ['-n', 'security', 'add-trusted-cert', '-d', '-r', 'trustRoot', '-k', '/Library/Keychains/System.keychain', ca.pem]);
  } else {
    tool('sudo', ['-n', 'cp', ca.pem, osTrustFile]);
    tool('sudo', ['-n', 'update-ca-certificates']);
  }
}

function removeOsTrust() {
  try {
    // macOS: `security remove-trusted-cert` hung a runner for 20 minutes; the runner is thrown away.
    if (process.platform !== 'darwin') {
      tool('sudo', ['-n', 'rm', '-f', osTrustFile]);
      tool('sudo', ['-n', 'update-ca-certificates', '--fresh']);
    }
  } catch {
    // The runner is thrown away.
  }
}

// ---- main -------------------------------------------------------------------------------------

function markdown(versions) {
  const lines = [`## One login E spike on ${PLATFORM}`, '', Object.entries(versions).map(([k, v]) => `- ${k}: \`${v}\``).join('\n'), ''];
  lines.push('| tool | mechanism | trust | command | verdict | detail |', '|---|---|---|---|---|---|');
  for (const r of results) {
    const mark = r.verdict === 'works' ? 'works' : r.verdict === 'not run' ? 'not run' : r.info ? 'fails (informational)' : 'fails';
    lines.push(`| ${r.tool} | ${r.mechanism} | ${r.trust} | \`${r.command}\` | **${mark}** | ${String(r.detail).replaceAll('|', '/').replaceAll('\n', ' ')} |`);
  }
  return `${lines.join('\n')}\n`;
}

async function main() {
  await startServers();
  const versions = {
    os: `${PLATFORM} ${os.release()}`,
    gh: versionOf('gh', ['--version']),
    glab: versionOf('glab', ['--version']),
    git: versionOf('git', ['--version']),
    go: versionOf('go', ['version']),
    node: process.version,
    openssl: versionOf('openssl', ['version']),
    cargo: versionOf('cargo', ['--version']),
    npm: versionOf('npm', ['--version']),
    python: versionOf('python3', ['--version']),
    port80: ports.eighty ? 'bound' : `not bound (${ports.eightyWhy})`,
  };
  console.log(JSON.stringify(versions, null, 2));
  flush = () => {
    fs.writeFileSync(path.join(outDir, `results-${PLATFORM}.json`), `${JSON.stringify({ versions, results, proxyLog }, null, 2)}\n`);
    fs.writeFileSync(path.join(outDir, `results-${PLATFORM}.md`), markdown(versions));
  };
  const all = cases();
  for (const c of all) await runCase(c, 'env').catch((e) => results.push({ os: PLATFORM, tool: c.tool, mechanism: c.mechanism, command: c.label, verdict: 'harness error', detail: String(e) }));
  const osCases = all.filter((c) => c.phases.includes('os'));
  if (IS_CI) {
    try {
      installOsTrust();
      for (const c of osCases) await runCase(c, 'os').catch((e) => results.push({ os: PLATFORM, tool: c.tool, mechanism: c.mechanism, command: c.label, verdict: 'harness error', detail: String(e) }));
    } catch (e) {
      results.push({ os: PLATFORM, tool: '-', mechanism: 'trusting the CA in the OS store', command: '-', verdict: 'not run', detail: String(e.stderr ?? e) });
    } finally {
      removeOsTrust();
    }
  } else {
    console.log('Not on CI: the OS trust store is left alone, so the OS-trust cases are not run.');
  }
  fs.writeFileSync(path.join(outDir, `results-${PLATFORM}.json`), `${JSON.stringify({ versions, results, proxyLog }, null, 2)}\n`);
  const md = markdown(versions);
  fs.writeFileSync(path.join(outDir, `results-${PLATFORM}.md`), md);
  console.log(md);
  if (process.env.GITHUB_STEP_SUMMARY) fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, md);
  for (const s of servers) s.close();
  process.exit(0);
}

main().catch((e) => {
  console.error('the spike harness failed:', e);
  process.exit(1);
});
