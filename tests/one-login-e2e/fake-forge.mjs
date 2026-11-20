// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// A fake forge for the one-login end-to-end test: one instance speaks one forge's shape, GitHub's
// or GitLab's, over TLS on loopback. It answers
//
//   - OAuth: the device flow, the authorization-code flow with PKCE, and the refresh grant with
//     single-use refresh tokens, on the paths each forge uses. Tokens expire on a clock the test
//     owns, so "the token ran out" is one call;
//   - git smart HTTP, by the standard `git http-backend`, for fetch and for push;
//   - REST and GraphQL subsets: what `gh pr list/create/view/merge`, `gh api`, and `glab mr
//     list/create/view/merge` and `glab api` ask. A field a client asks for that the fake does not
//     know answers `null`; a path it does not know answers 404 naming it, so a test that needs
//     more fails on exactly that.
//
// Every request is logged with the credentials it carried, and a request that carries a string the
// test registered with `mustNotSee` (the sentinel a chat holds) is flagged: the real forge must
// see only a real access token.

import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import https from 'node:https';
import os from 'node:os';
import path from 'node:path';
import { execute } from './graphql-lite.mjs';

export const ACCOUNT = 'alice';
export const CLIENT_ID = 'fake-client-id'; // a public client id, as purlis's sign-in uses

const rand = (n = 18) => crypto.randomBytes(n).toString('base64url');
const sha256url = (s) => crypto.createHash('sha256').update(s).digest('base64url');

const isolated = { ...process.env, GIT_CONFIG_GLOBAL: os.devNull, GIT_CONFIG_NOSYSTEM: '1', GIT_TERMINAL_PROMPT: '0' };
function git(args, opts = {}) {
  return execFileSync('git', ['-c', 'user.name=forge', '-c', `user.email=${['forge', 'fake.invalid'].join('@')}`, ...args], {
    encoding: 'utf8',
    env: isolated,
    stdio: ['ignore', 'pipe', 'pipe'],
    ...opts,
  }).trim();
}

/**
 * @param {{ kind: 'github' | 'gitlab', root: string, tls: { key: Buffer, cert: Buffer }, clock: { now(): number },
 *           expiresIn: number | null, host: string, repos: string[] }} options
 */
export async function startFakeForge({ kind, root, tls, clock, expiresIn, host, repos: repoNames, publicRepos = [] }) {
  const gh = kind === 'github';
  const prefix = gh ? 'fake-gh-' : 'fake-gl-';
  const log = [];
  const mustNotSee = new Set();
  /** Every secret the forge ever issued: a test scans for these in what a chat can see. */
  const issued = new Set();
  const access = new Map(); // token -> { refresh, expiresAt, revoked }
  const refreshes = new Map(); // refresh token -> { used }
  const devices = new Map(); // device_code -> { userCode, approved }
  const codes = new Map(); // authorization code -> { challenge }
  const merges = []; // { repo, number, head, result }
  const grants = []; // the token endpoint's grants, in order: 'device_code', 'refresh_token', ...
  let nextProjectId = 1000;

  // ---- repositories ---------------------------------------------------------------------------

  const repos = new Map();
  for (const full of [...repoNames, ...publicRepos]) {
    const [owner, name] = full.split('/');
    const bare = path.join(root, owner, `${name}.git`);
    const src = fs.mkdtempSync(path.join(root, 'seed-'));
    git(['init', '-q', '-b', 'main'], { cwd: src });
    fs.writeFileSync(path.join(src, 'README.md'), `# ${full}\n`);
    git(['add', '-A'], { cwd: src });
    git(['commit', '-q', '-m', 'first commit'], { cwd: src });
    fs.mkdirSync(path.dirname(bare), { recursive: true });
    git(['clone', '-q', '--bare', src, bare]);
    git(['config', 'http.receivepack', 'true'], { cwd: bare });
    nextProjectId += 1;
    repos.set(full, { id: nextProjectId, owner, name, full, bare, prs: [], isPublic: publicRepos.includes(full) });
  }
  const refSha = (repo, ref) => {
    try {
      return git(['--git-dir', repo.bare, 'rev-parse', '--verify', '-q', ref]);
    } catch {
      return null;
    }
  };
  const isAncestor = (repo, a, b) => {
    try {
      git(['--git-dir', repo.bare, 'merge-base', '--is-ancestor', a, b]);
      return true;
    } catch {
      return false;
    }
  };
  const repoOf = (idOrPath) => {
    const key = decodeURIComponent(String(idOrPath));
    return repos.get(key) ?? [...repos.values()].find((r) => String(r.id) === key) ?? null;
  };

  // ---- pull and merge requests ----------------------------------------------------------------

  const headSha = (repo, pr) => refSha(repo, `refs/heads/${pr.head}`) ?? pr.mergedSha;
  const webUrl = (repo) => `https://${host}/${repo.full}`;
  const iso = () => new Date(clock.now()).toISOString();

  function openPr(repo, { title, body, head, base }) {
    const pr = { number: repo.prs.length + 1, title, body: body ?? '', head, base, state: 'open', mergedSha: null, createdAt: iso() };
    repo.prs.push(pr);
    return pr;
  }

  function mergePr(repo, pr, sha) {
    const head = headSha(repo, pr);
    if (pr.state === 'merged') return { status: 405, body: { message: 'already merged' } };
    if (sha && sha !== head) return { status: 409, body: { message: 'Head branch was modified. Review and try the merge again.' } };
    const base = refSha(repo, `refs/heads/${pr.base}`);
    let result = head;
    if (!isAncestor(repo, base, head)) {
      try {
        const tree = git(['--git-dir', repo.bare, 'merge-tree', '--write-tree', base, head]).split('\n')[0];
        result = git(['--git-dir', repo.bare, 'commit-tree', tree, '-p', base, '-p', head, '-m', `Merge ${pr.head} into ${pr.base}`]);
      } catch {
        return { status: 405, body: { message: 'Pull Request is not mergeable' } };
      }
    }
    git(['--git-dir', repo.bare, 'update-ref', `refs/heads/${pr.base}`, result]);
    pr.state = 'merged';
    pr.mergedSha = result;
    merges.push({ repo: repo.full, number: pr.number, head, result });
    return { status: 200, body: gh ? { merged: true, sha: result, message: 'Pull Request successfully merged' } : mergedMr(repo, pr) };
  }

  // GitHub's shapes
  const ghPr = (repo, pr) => ({
    id: pr.number + 5000,
    node_id: `PR_${pr.number}`,
    number: pr.number,
    state: pr.state === 'open' ? 'open' : 'closed',
    merged: pr.state === 'merged',
    title: pr.title,
    body: pr.body,
    html_url: `${webUrl(repo)}/pull/${pr.number}`,
    url: `https://${host}/repos/${repo.full}/pulls/${pr.number}`,
    user: { login: ACCOUNT },
    created_at: pr.createdAt,
    head: { ref: pr.head, sha: headSha(repo, pr), repo: { full_name: repo.full } },
    base: { ref: pr.base, sha: refSha(repo, `refs/heads/${pr.base}`), repo: { full_name: repo.full } },
    draft: false,
  });
  const ghRepoJson = (repo) => ({
    id: repo.id,
    node_id: `R_${repo.id}`,
    name: repo.name,
    full_name: repo.full,
    private: true,
    owner: { login: repo.owner },
    default_branch: 'main',
    html_url: webUrl(repo),
    clone_url: `${webUrl(repo)}.git`,
    permissions: { admin: true, push: true, pull: true },
  });
  const ghNode = (repo, pr) => ({
    __typename: 'PullRequest',
    id: `PR_${pr.number}`,
    number: pr.number,
    title: pr.title,
    body: pr.body,
    state: pr.state === 'open' ? 'OPEN' : 'MERGED',
    url: `${webUrl(repo)}/pull/${pr.number}`,
    headRefName: pr.head,
    headRefOid: headSha(repo, pr),
    baseRefName: pr.base,
    baseRefOid: refSha(repo, `refs/heads/${pr.base}`),
    isDraft: false,
    isCrossRepository: false,
    maintainerCanModify: false,
    mergeable: 'MERGEABLE',
    mergeStateStatus: 'CLEAN',
    reviewDecision: null,
    createdAt: pr.createdAt,
    updatedAt: pr.createdAt,
    closedAt: pr.state === 'merged' ? pr.createdAt : null,
    mergedAt: pr.state === 'merged' ? pr.createdAt : null,
    additions: 1,
    deletions: 0,
    changedFiles: 1,
    author: { __typename: 'User', login: ACCOUNT },
    headRepository: { id: `R_${repo.id}`, name: repo.name, nameWithOwner: repo.full },
    headRepositoryOwner: { login: repo.owner },
    labels: { totalCount: 0, nodes: [] },
    comments: { totalCount: 0, nodes: [] },
    reviews: { totalCount: 0, nodes: [] },
    commits: () => ({ totalCount: 1, nodes: [{ commit: { oid: headSha(repo, pr), messageHeadline: pr.title, statusCheckRollup: null } }] }),
  });
  const connection = (nodes) => ({ totalCount: nodes.length, nodes, edges: nodes.map((node) => ({ node })), pageInfo: { hasNextPage: false, endCursor: null } });
  const ghRepoNode = (repo) => ({
    __typename: 'Repository',
    id: `R_${repo.id}`,
    databaseId: repo.id,
    name: repo.name,
    nameWithOwner: repo.full,
    owner: { __typename: 'User', id: 'U_owner', login: repo.owner },
    url: webUrl(repo),
    sshUrl: `git@${host}:${repo.full}.git`,
    description: '',
    isPrivate: true,
    isFork: false,
    isArchived: false,
    isEmpty: false,
    visibility: 'PRIVATE',
    parent: null,
    defaultBranchRef: { name: 'main', target: { oid: refSha(repo, 'refs/heads/main') } },
    hasIssuesEnabled: true,
    hasWikiEnabled: false,
    viewerPermission: 'ADMIN',
    viewerCanAdminister: true,
    mergeCommitAllowed: true,
    squashMergeAllowed: true,
    rebaseMergeAllowed: true,
    autoMergeAllowed: false,
    deleteBranchOnMerge: false,
    pullRequestTemplates: [],
    issueTemplates: [],
    pullRequests: (a) => {
      let list = repo.prs;
      const states = a.states ? [].concat(a.states) : null;
      if (states) list = list.filter((p) => states.includes(p.state === 'open' ? 'OPEN' : 'MERGED'));
      if (a.headRefName) list = list.filter((p) => p.head === a.headRefName);
      if (a.baseRefName) list = list.filter((p) => p.base === a.baseRefName);
      return connection([...list].reverse().map((p) => ghNode(repo, p)));
    },
    pullRequest: (a) => {
      const pr = repo.prs.find((p) => p.number === a.number);
      return pr ? ghNode(repo, pr) : null;
    },
  });
  const ghGraphql = {
    query: {
      __type: () => ({ fields: [], inputFields: [], enumValues: [] }),
      viewer: { __typename: 'User', login: ACCOUNT, id: 'U_1', name: 'Alice' },
      repository: (a) => {
        const repo = repos.get(`${a.owner}/${a.name}`);
        return repo ? ghRepoNode(repo) : null;
      },
    },
    mutation: {
      createPullRequest: ({ input }) => {
        const repo = [...repos.values()].find((r) => `R_${r.id}` === input.repositoryId);
        if (!repo || !refSha(repo, `refs/heads/${input.headRefName}`)) return null;
        return { pullRequest: ghNode(repo, openPr(repo, { title: input.title, body: input.body, head: input.headRefName, base: input.baseRefName })) };
      },
      mergePullRequest: ({ input }) => {
        const n = Number(String(input.pullRequestId).replace('PR_', ''));
        for (const repo of repos.values()) {
          const pr = repo.prs.find((p) => p.number === n);
          if (pr) {
            mergePr(repo, pr, input.expectedHeadOid);
            return { pullRequest: ghNode(repo, pr) };
          }
        }
        return null;
      },
    },
  };

  // GitLab's shapes
  const glProject = (repo) => ({
    id: repo.id,
    name: repo.name,
    path: repo.name,
    path_with_namespace: repo.full,
    name_with_namespace: repo.full.replace('/', ' / '),
    default_branch: 'main',
    visibility: 'private',
    web_url: webUrl(repo),
    http_url_to_repo: `${webUrl(repo)}.git`,
    namespace: { id: 1, path: repo.owner, full_path: repo.owner, kind: 'group' },
    permissions: { project_access: { access_level: 40 }, group_access: null },
    merge_requests_enabled: true,
    issues_enabled: true,
  });
  const glMr = (repo, pr) => ({
    id: pr.number + 7000,
    iid: pr.number,
    project_id: repo.id,
    title: pr.title,
    description: pr.body,
    state: pr.state === 'open' ? 'opened' : 'merged',
    source_branch: pr.head,
    target_branch: pr.base,
    sha: headSha(repo, pr),
    web_url: `${webUrl(repo)}/-/merge_requests/${pr.number}`,
    author: { id: 1, username: ACCOUNT, name: 'Alice' },
    user: { can_merge: true },
    references: { short: `!${pr.number}`, relative: `!${pr.number}`, full: `${repo.full}!${pr.number}` },
    head_pipeline: null,
    pipeline: null,
    milestone: null,
    merged_by: null,
    closed_by: null,
    user_notes_count: 0,
    upvotes: 0,
    downvotes: 0,
    merge_status: 'can_be_merged',
    detailed_merge_status: 'mergeable',
    has_conflicts: false,
    draft: false,
    work_in_progress: false,
    labels: [],
    assignees: [],
    reviewers: [],
    created_at: pr.createdAt,
    updated_at: pr.createdAt,
    source_project_id: repo.id,
    target_project_id: repo.id,
  });
  const mergedMr = (repo, pr) => glMr(repo, pr);

  // ---- OAuth ----------------------------------------------------------------------------------

  function mint() {
    const token = `${prefix}${rand()}`;
    const refresh = expiresIn == null ? null : `${prefix}r-${rand()}`;
    access.set(token, { refresh, expiresAt: expiresIn == null ? null : clock.now() + expiresIn * 1000, revoked: false });
    issued.add(token);
    if (refresh) {
      refreshes.set(refresh, { used: false, access: token });
      issued.add(refresh);
    }
    const reply = { access_token: token, token_type: gh ? 'bearer' : 'Bearer', scope: gh ? 'repo,read:org' : 'api', created_at: Math.floor(clock.now() / 1000) };
    if (refresh) Object.assign(reply, { refresh_token: refresh, expires_in: expiresIn, ...(gh ? { refresh_token_expires_in: 15_811_200 } : {}) });
    return reply;
  }
  const oauthError = (error, status = 400) => ({ status: gh ? 200 : status, body: { error, error_description: error } });
  const deviceUrl = gh ? '/login/device/code' : '/oauth/authorize_device';
  const tokenUrl = gh ? '/login/oauth/access_token' : '/oauth/token';
  const authorizeUrl = gh ? '/login/oauth/authorize' : '/oauth/authorize';

  function oauth(pathname, params) {
    if (pathname === deviceUrl) {
      if (params.client_id !== CLIENT_ID) return oauthError('invalid_client', 401);
      const device = `${prefix}dev-${rand()}`;
      const userCode = `${rand(3).slice(0, 4)}-${rand(3).slice(0, 4)}`.toUpperCase();
      devices.set(device, { userCode, approved: false });
      issued.add(device);
      return { status: 200, body: { device_code: device, user_code: userCode, verification_uri: `https://${host}${gh ? '/login/device' : '/oauth/device'}`, expires_in: 900, interval: 1 } };
    }
    if (pathname === authorizeUrl) {
      // The person's browser, as a request: the authorization-code flow with PKCE, approved at once.
      if (params.client_id !== CLIENT_ID || params.code_challenge_method !== 'S256') return { status: 400, body: { error: 'invalid_request' } };
      const code = `${prefix}code-${rand()}`;
      codes.set(code, { challenge: params.code_challenge });
      const to = new URL(params.redirect_uri);
      to.searchParams.set('code', code);
      if (params.state) to.searchParams.set('state', params.state);
      return { status: 302, headers: { location: to.toString() }, body: {} };
    }
    if (pathname !== tokenUrl) return null;
    if (params.client_id !== CLIENT_ID) return oauthError('invalid_client', 401);
    grants.push(params.grant_type);
    if (params.grant_type === 'urn:ietf:params:oauth:grant-type:device_code') {
      const d = devices.get(params.device_code);
      if (!d) return oauthError('bad_device_code');
      if (!d.approved) return oauthError('authorization_pending');
      devices.delete(params.device_code);
      return { status: 200, body: mint() };
    }
    if (params.grant_type === 'authorization_code') {
      const c = codes.get(params.code);
      if (!c || !params.code_verifier || sha256url(params.code_verifier) !== c.challenge) return oauthError('invalid_grant');
      codes.delete(params.code);
      return { status: 200, body: mint() };
    }
    if (params.grant_type === 'refresh_token') {
      const r = refreshes.get(params.refresh_token);
      if (!r || r.used) return oauthError(gh ? 'bad_refresh_token' : 'invalid_grant');
      r.used = true;
      const old = access.get(r.access);
      if (old) old.revoked = true; // each refresh replaces both tokens
      return { status: 200, body: mint() };
    }
    return oauthError('unsupported_grant_type');
  }

  // ---- the API --------------------------------------------------------------------------------

  function credentialsOf(headers) {
    const found = [];
    const auth = headers.authorization ?? '';
    if (/^basic /i.test(auth)) {
      const decoded = Buffer.from(auth.slice(6), 'base64').toString();
      found.push(decoded.slice(decoded.indexOf(':') + 1)); // the password half; the user name is no secret
    } else if (auth) found.push(auth.replace(/^(token|bearer) /i, ''));
    if (headers['private-token']) found.push(headers['private-token']);
    return found;
  }
  const tokenOk = (token) => {
    const t = access.get(token);
    return Boolean(t && !t.revoked && (t.expiresAt == null || clock.now() < t.expiresAt));
  };

  const json = (status, body, headers) => ({ status, body, headers });
  const notFound = (req) => json(404, { message: `fake forge: no answer for ${req.method} ${req.path}` });

  function githubApi(req, body) {
    const p = req.path.replace(/^\/api\/v3(?=\/)/, '');
    let m;
    if (req.method === 'GET' && p === '/user') return json(200, { login: ACCOUNT, id: 1, type: 'User' }, { 'x-oauth-scopes': 'repo, read:org' });
    if (req.method === 'POST' && p === '/graphql') return json(200, execute(body.query ?? '', body.variables ?? {}, ghGraphql));
    if ((m = /^\/repos\/([^/]+)\/([^/]+)(\/.*)?$/.exec(p))) {
      const repo = repos.get(`${m[1]}/${m[2]}`);
      const rest = m[3] ?? '';
      if (!repo) return json(404, { message: 'Not Found' });
      if (req.method === 'GET' && rest === '') return json(200, ghRepoJson(repo));
      if (req.method === 'GET' && rest === '/pulls') {
        const state = req.query.get('state') ?? 'open';
        return json(200, repo.prs.filter((x) => state === 'all' || (state === 'open') === (x.state === 'open')).map((x) => ghPr(repo, x)));
      }
      if (req.method === 'POST' && rest === '/pulls') {
        if (!body.title || !body.head || !body.base || !refSha(repo, `refs/heads/${body.head}`)) return json(422, { message: 'Validation Failed', errors: [{ resource: 'PullRequest', code: 'invalid' }] });
        return json(201, ghPr(repo, openPr(repo, body)));
      }
      if ((m = /^\/pulls\/(\d+)(\/merge)?$/.exec(rest))) {
        const pr = repo.prs.find((x) => x.number === Number(m[1]));
        if (!pr) return json(404, { message: 'Not Found' });
        if (req.method === 'GET' && !m[2]) return json(200, ghPr(repo, pr));
        if (req.method === 'PUT' && m[2]) {
          const r = mergePr(repo, pr, body.sha);
          return json(r.status, r.body);
        }
      }
    }
    return notFound(req);
  }

  function gitlabApi(req, body) {
    const p = req.path.replace(/^\/api\/v4/, '');
    let m;
    if (req.method === 'GET' && p === '/user') return json(200, { id: 1, username: ACCOUNT, name: 'Alice', state: 'active' });
    if (req.method === 'GET' && p === '/version') return json(200, { version: '17.9.0', revision: 'fake' });
    if ((m = /^\/projects\/([^/]+)(\/.*)?$/.exec(p))) {
      const repo = repoOf(m[1]);
      const rest = m[2] ?? '';
      if (!repo) return json(404, { message: '404 Project Not Found' });
      if (req.method === 'GET' && rest === '') return json(200, glProject(repo));
      if (req.method === 'GET' && (m = /^\/repository\/branches\/(.+)$/.exec(rest))) {
        const sha = refSha(repo, `refs/heads/${decodeURIComponent(m[1])}`);
        return sha ? json(200, { name: decodeURIComponent(m[1]), commit: { id: sha }, default: m[1] === 'main' }) : json(404, { message: '404 Branch Not Found' });
      }
      if (req.method === 'GET' && rest === '/merge_requests') {
        const state = req.query.get('state') ?? 'opened';
        return json(200, repo.prs.filter((x) => state === 'all' || (state === 'opened') === (x.state === 'open')).map((x) => glMr(repo, x)));
      }
      if (req.method === 'POST' && rest === '/merge_requests') {
        const f = { title: body.title, body: body.description, head: body.source_branch, base: body.target_branch };
        if (!f.title || !f.head || !f.base || !refSha(repo, `refs/heads/${f.head}`)) return json(400, { message: 'invalid merge request' });
        return json(201, glMr(repo, openPr(repo, f)));
      }
      if ((m = /^\/merge_requests\/(\d+)(\/merge)?$/.exec(rest))) {
        const pr = repo.prs.find((x) => x.number === Number(m[1]));
        if (!pr) return json(404, { message: '404 Not found' });
        if (req.method === 'GET' && !m[2]) return json(200, glMr(repo, pr));
        if (req.method === 'PUT' && m[2]) {
          const r = mergePr(repo, pr, body.sha);
          return json(r.status, r.body);
        }
      }
    }
    return notFound(req);
  }

  // ---- git over HTTP, by the standard CGI -----------------------------------------------------

  function gitBackend(req, raw, res, entry) {
    const env = {
      PATH: process.env.PATH,
      GIT_PROJECT_ROOT: root,
      GIT_HTTP_EXPORT_ALL: '1',
      GIT_CONFIG_GLOBAL: os.devNull,
      GIT_CONFIG_NOSYSTEM: '1',
      REQUEST_METHOD: req.method,
      PATH_INFO: req.path,
      QUERY_STRING: req.rawQuery,
      CONTENT_TYPE: req.headers['content-type'] ?? '',
      CONTENT_LENGTH: String(raw.length),
      REMOTE_USER: ACCOUNT,
      REMOTE_ADDR: '127.0.0.1',
      ...(req.headers['content-encoding'] ? { HTTP_CONTENT_ENCODING: req.headers['content-encoding'] } : {}),
      ...(req.headers['git-protocol'] ? { HTTP_GIT_PROTOCOL: req.headers['git-protocol'] } : {}),
    };
    const child = spawn('git', ['http-backend'], { env });
    const out = [];
    child.stdout.on('data', (c) => out.push(c));
    child.stderr.on('data', () => {});
    child.on('close', () => {
      const buf = Buffer.concat(out);
      const split = buf.indexOf('\r\n\r\n');
      const head = buf.subarray(0, split < 0 ? 0 : split).toString().split('\r\n');
      const headers = { connection: 'close' };
      let status = 200;
      for (const line of head) {
        const i = line.indexOf(':');
        if (i < 0) continue;
        const k = line.slice(0, i).toLowerCase();
        const v = line.slice(i + 1).trim();
        if (k === 'status') status = Number(v.split(' ')[0]);
        else headers[k] = v;
      }
      entry.status = status;
      res.writeHead(status, headers);
      res.end(split < 0 ? buf : buf.subarray(split + 4));
    });
    child.stdin.on('error', () => {});
    child.stdin.end(raw);
  }

  // ---- the server -----------------------------------------------------------------------------

  function handler(rq, res) {
    const url = new URL(rq.url, 'https://forge.invalid');
    const req = { method: rq.method, path: url.pathname, query: url.searchParams, rawQuery: url.search.slice(1), headers: rq.headers };
    const chunks = [];
    rq.on('data', (c) => chunks.push(c));
    rq.on('end', () => {
      const raw = Buffer.concat(chunks);
      const credentials = credentialsOf(rq.headers);
      const entry = {
        method: req.method,
        path: req.path + (req.rawQuery ? `?${req.rawQuery}` : ''),
        host: rq.headers.host,
        credentials,
        status: 0,
        sawForbidden: [...mustNotSee].some((s) => JSON.stringify(rq.headers).includes(s) || raw.includes(s) || rq.url.includes(s)),
      };
      log.push(entry);
      const send = (r) => {
        entry.status = r.status;
        res.writeHead(r.status, { 'content-type': 'application/json', connection: 'close', ...r.headers });
        res.end(JSON.stringify(r.body));
      };
      const ct = rq.headers['content-type'] ?? '';
      let body = {};
      let params = {};
      try {
        if (raw.length && ct.includes('json')) body = JSON.parse(raw.toString());
        else if (raw.length && ct.includes('x-www-form-urlencoded')) body = Object.fromEntries(new URLSearchParams(raw.toString()));
        params = { ...Object.fromEntries(req.query), ...body };
      } catch {
        return send({ status: 400, body: { message: 'unreadable body' } });
      }

      const oauthReply = oauth(req.path, params);
      if (oauthReply) return send(oauthReply);
      if (req.method === 'POST' && req.path === '/__fake/approve') {
        // The person's browser pressing "authorize" for a user code.
        const d = [...devices.values()].find((x) => x.userCode === params.user_code);
        if (d) d.approved = true;
        return send({ status: d ? 200 : 404, body: {} });
      }

      if (!credentials.some(tokenOk)) {
        // Anonymous: only a public repository's git reads answer. Everything else is a 401, as a real forge's is.
        const g = /^\/([^/]+)\/([^/]+)\.git\//.exec(req.path);
        const pub = g && repos.get(`${g[1]}/${g[2]}`)?.isPublic;
        const push = req.path.endsWith('/git-receive-pack') || req.query.get('service') === 'git-receive-pack';
        if (pub && !push) return gitBackend(req, raw, res, entry);
        const r = { status: 401, body: { message: gh ? 'Bad credentials' : '401 Unauthorized' }, headers: { 'www-authenticate': 'Basic realm="forge"' } };
        return send(r);
      }
      if (/^\/[^/]+\/[^/]+\.git\//.test(req.path)) return gitBackend(req, raw, res, entry);
      return send(gh ? githubApi(req, body) : gitlabApi(req, body));
    });
  }

  const server = https.createServer(tls, handler);
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

  return {
    kind,
    host,
    port: server.address().port,
    log,
    issued,
    grants,
    merges,
    mustNotSee,
    repos,
    refSha: (full, ref) => refSha(repos.get(full), ref),
    /** What the person's browser does at the verification page. */
    approve: (userCode) => {
      const d = [...devices.values()].find((x) => x.userCode === userCode);
      if (!d) throw new Error(`no device with user code ${userCode}`);
      d.approved = true;
    },
    /** The forge revokes every access token now, as if the person had revoked it or it had run out. */
    revokeAccessTokens: () => access.forEach((t) => (t.revoked = true)),
    /** A teammate pushes a commit to a branch of a repo. */
    teammatePush: (full, branch, file) => {
      const repo = repos.get(full);
      const work = fs.mkdtempSync(path.join(root, 'mate-'));
      git(['clone', '-q', repo.bare, work]);
      fs.writeFileSync(path.join(work, file), `from a teammate\n`);
      git(['add', '-A'], { cwd: work });
      git(['commit', '-q', '-m', `teammate adds ${file}`], { cwd: work });
      git(['push', '-q', 'origin', `HEAD:refs/heads/${branch}`], { cwd: work });
    },
    prs: (full) => repos.get(full).prs.map((p) => ({ ...p, head: p.head, sha: headSha(repos.get(full), p) })),
    close: () => server.close(),
  };
}
