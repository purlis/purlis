#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **One login, slice I: the end-to-end test that proves the scenario** (ADR 0128).
//
//   node tests/one-login-e2e/run.mjs [--out <dir>]
//
// Someone makes a repo. Anyone with access signs in to their own forge account in purlis once,
// clones the repo as a project, and a chat runs `gh`, `glab` and native `git` with no manual
// command. This runs that, against a local fake forge of each shape (GitHub's and GitLab's),
// with the real `gh`, `glab` and `git`, and fails if any step does:
//
//   sign in once -> the token is in the keyring -> clone as a project -> it is recorded ->
//   a chat: gh pr create/list/view, gh api, glab mr create/list/view, git fetch/pull/push to a
//   branch it created -> refused: a push to main or a tag, `gh pr merge` (it becomes a held ask),
//   another repo, `gh auth token` -> the token renews unattended -> the chat never saw a token.
//
// Slices of ADR 0128 not merged yet are stood in for by `seams.mjs`, each marked
// `TODO(slice X)`; the scenario below never changes when one is swapped for the product's.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { startFakeForge } from './fake-forge.mjs';
import { expect, filesUnder, makeCa, manualClock, reporter, tail, tool } from './harness.mjs';
import { createApp } from './seams.mjs';

const PLATFORM = process.platform === 'darwin' ? 'macOS' : process.platform === 'linux' ? 'Linux' : process.platform;
const outDir = (() => {
  const i = process.argv.indexOf('--out');
  return i > 0 ? path.resolve(process.argv[i + 1]) : fs.mkdtempSync(path.join(os.tmpdir(), 'one-login-e2e-out-'));
})();
fs.mkdirSync(outDir, { recursive: true });
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'ol-'));

const HOUR = 3600 * 1000;
const rp = reporter();
const ok = (r, what) => expect(r.code === 0, `${what} should work; ${tail(r)}`);
// Refused means purlis refused it: a non-zero exit AND a refusal or hold in the proxy's audit for that command.
// (git's first, unauthenticated request of a push is answered 401 to ask for the helper's credential; that is no refusal.)
const denied = (r, what) => {
  expect(r.code !== 0, `${what} should be refused, and it exited 0: ${r.stdout.trim().slice(0, 200)}`);
  expect((r.refusals ?? []).length > 0, `${what} failed, but purlis did not refuse it (the audit has no refusal for it): ${tail(r)}`);
};

// A repo outside the project is unreachable: purlis refuses it, or (the anonymous route, ADR 0128 D3i) serves it as the public
// read it is, with no credential, and the forge refuses a private repo to an anonymous caller. Either way the check below
// (no credentialed request for it ever reaches a forge) is the one that matters.
const unreachable = (r, what) => expect(r.code !== 0, `${what} should not work, and it exited 0: ${r.stdout.trim().slice(0, 200)}`);

async function main() {
  const clock = manualClock();
  const ca = makeCa(work);
  const leaf = ca.leafFor(['127.0.0.1', 'localhost']);
  const startForge = (kind, host, expiresIn) => {
    const root = path.join(work, `forge-${kind}`);
    fs.mkdirSync(root, { recursive: true });
    return startFakeForge({ kind, root, tls: leaf, clock, expiresIn, host, repos: ['o/r', 'o/other'], publicRepos: ['o/public'] });
  };
  // purlis's github.com registration is an OAuth App: its tokens do not expire and have no refresh token (ADR 0128 D2).
  // GitLab's access token lasts 2 hours, and each refresh replaces both tokens.
  const github = await startForge('github', 'github.com', null);
  const gitlab = await startForge('gitlab', 'gitlab.com', 2 * 3600);
  const app = await createApp({ work, ca, clock, forges: { github, gitlab } });

  const versions = Object.fromEntries(
    [['gh', ['--version']], ['glab', ['--version']], ['git', ['--version']], ['node', ['--version']]].map(([c, a]) => {
      try {
        return [c, tool(c, a).split('\n')[0].trim()];
      } catch {
        return [c, 'not installed'];
      }
    }),
  );
  console.log(JSON.stringify({ os: `${PLATFORM} ${os.release()}`, ...versions }, null, 2));

  let chat;
  const chatRun = async (cmd, args, cwd) => {
    const mark = app.audit.length;
    const r = await chat.exec(cmd, args, cwd);
    r.refusals = app.audit.slice(mark).filter((a) => (a.verdict === 'refuse' || a.verdict === 'hold') && !/challenged/.test(a.reason));
    rp.outputs.push(r.stdout, r.stderr);
    return r;
  };
  const sh = (cwd, script) => chatRun('sh', ['-c', script], cwd);
  let GH;
  let GL;

  // ---- sign in once, per forge host ----------------------------------------------------------

  await rp.step('sign in to github.com once: the device flow, the token in the keyring', async () => {
    const who = await app.signIn(github);
    expect(who === 'alice', `signed in as ${who}`);
    const k = app.keyring.get('github.com');
    expect(k.access && !k.refresh && !k.expiresAt, 'the keyring holds an access token with no refresh token and no expiry (an OAuth App\'s)');
    expect(github.grants.includes('urn:ietf:params:oauth:grant-type:device_code'), 'the forge saw a device grant');
  });
  await rp.step('sign in to gitlab.com once: the device flow, the token in the keyring', async () => {
    const who = await app.signIn(gitlab);
    expect(who === 'alice', `signed in as ${who}`);
    const k = app.keyring.get('gitlab.com');
    expect(k.access && k.refresh && k.expiresAt, 'the keyring holds an access token, a refresh token and an expiry');
  });
  await rp.step('the authorization-code flow with PKCE refuses a wrong verifier and accepts the right one (both forges)', async () => {
    for (const f of [github, gitlab]) {
      const r = await app.signInWithPkce(f);
      expect(r.wrongVerifierRefused, `${f.kind}: a wrong code verifier must be refused`);
      expect(r.token, `${f.kind}: the right verifier must get a token`);
    }
  });

  // ---- clone as a project, recorded ------------------------------------------------------------

  await rp.step(`record both projects, then start the chat${app.bridged ? ' (slice A\'s ForgeProxy enrols it and mints its sentinel)' : ''}`, async () => {
    const g = await app.recordProject(github, 'o', 'r');
    expect(g.id === github.repos.get('o/r').id && app.recorded.has('github.com/o/r'), `the project is recorded with the forge's own id: ${JSON.stringify(g)}`);
    const l = await app.recordProject(gitlab, 'o', 'r');
    expect(l.id === gitlab.repos.get('o/r').id && app.recorded.has('gitlab.com/o/r'), `the project is recorded with the forge's own id: ${JSON.stringify(l)}`);
    await app.enrol();
    for (const f of [github, gitlab]) f.mustNotSee.add(app.sentinel);
    chat = app.makeChat();
  });
  await rp.step('clone github.com/o/r as a project: the remote is unchanged', async () => {
    ok(await chatRun('git', ['clone', 'https://github.com/o/r.git', 'github-r']), 'git clone');
    GH = path.join(chat.projects, 'github-r');
    const r = await chatRun('git', ['remote', '-v'], GH);
    expect(r.stdout.includes('https://github.com/o/r.git (fetch)'), `git remote -v should say github.com: ${r.stdout}`);
  });
  await rp.step('clone gitlab.com/o/r as a project: the remote is unchanged', async () => {
    ok(await chatRun('git', ['clone', 'https://gitlab.com/o/r.git', 'gitlab-r']), 'git clone');
    GL = path.join(chat.projects, 'gitlab-r');
    const r = await chatRun('git', ['remote', '-v'], GL);
    expect(r.stdout.includes('https://gitlab.com/o/r.git (fetch)'), `git remote -v should say gitlab.com: ${r.stdout}`);
  });

  // ---- a chat on GitHub --------------------------------------------------------------------------

  await rp.step('git push to a branch the chat created (github.com)', async () => {
    ok(await sh(GH, 'git checkout -q -b chat/feature && echo one > chat.txt && git add chat.txt && git commit -q -m "chat adds a file"'), 'commit on a new branch');
    ok(await chatRun('git', ['push', '-u', 'origin', 'chat/feature'], GH), 'git push');
    const local = (await chatRun('git', ['rev-parse', 'HEAD'], GH)).stdout.trim();
    expect(github.refSha('o/r', 'refs/heads/chat/feature') === local, 'the forge has the chat branch at the local tip');
  });
  await rp.step('gh pr create (github.com)', async () => {
    const r = await chatRun('gh', ['pr', 'create', '--head', 'chat/feature', '--base', 'main', '--title', 'Chat feature', '--body', 'From the chat'], GH);
    ok(r, 'gh pr create');
    expect(r.stdout.includes('/pull/1'), `gh should print the pull request's URL: ${r.stdout}`);
    expect(github.prs('o/r').length === 1, 'the forge has one pull request');
  });
  await rp.step('gh pr list and gh pr view (github.com)', async () => {
    const list = await chatRun('gh', ['pr', 'list'], GH);
    ok(list, 'gh pr list');
    expect(list.stdout.includes('Chat feature'), `the list names the pull request: ${list.stdout}`);
    const view = await chatRun('gh', ['pr', 'view', '1'], GH);
    ok(view, 'gh pr view 1');
    expect(view.stdout.includes('Chat feature'), `the view names the pull request: ${view.stdout}`);
  });
  await rp.step('gh api on the repo (github.com)', async () => {
    const repo = await chatRun('gh', ['api', 'repos/o/r'], GH);
    ok(repo, 'gh api repos/o/r');
    expect(JSON.parse(repo.stdout).full_name === 'o/r', `the repo answers: ${repo.stdout.slice(0, 120)}`);
    const pulls = await chatRun('gh', ['api', 'repos/o/r/pulls'], GH);
    ok(pulls, 'gh api repos/o/r/pulls');
    expect(JSON.parse(pulls.stdout).length === 1, 'the pull request list has one');
  });
  await rp.step('git fetch, git pull and a second push to the chat branch (github.com)', async () => {
    github.teammatePush('o/r', 'main', 'teammate.txt');
    ok(await chatRun('git', ['fetch', 'origin'], GH), 'git fetch');
    ok(await sh(GH, 'git checkout -q main'), 'checkout main');
    ok(await chatRun('git', ['pull', '--ff-only', 'origin', 'main'], GH), 'git pull');
    expect(fs.existsSync(path.join(GH, 'teammate.txt')), 'the pull brought the teammate\'s file');
    ok(await sh(GH, 'git checkout -q chat/feature && echo two >> chat.txt && git commit -q -am "chat edits the file"'), 'second commit');
    ok(await chatRun('git', ['push', 'origin', 'chat/feature'], GH), 'a second push to its own branch');
  });

  // ---- a chat on GitLab --------------------------------------------------------------------------

  await rp.step('git push to a branch the chat created (gitlab.com)', async () => {
    ok(await sh(GL, 'git checkout -q -b chat/feature && echo one > chat.txt && git add chat.txt && git commit -q -m "chat adds a file"'), 'commit on a new branch');
    ok(await chatRun('git', ['push', '-u', 'origin', 'chat/feature'], GL), 'git push');
    expect(gitlab.refSha('o/r', 'refs/heads/chat/feature') !== null, 'the forge has the chat branch');
  });
  await rp.step('glab mr create (gitlab.com)', async () => {
    const r = await chatRun('glab', ['mr', 'create', '--source-branch', 'chat/feature', '--target-branch', 'main', '--title', 'Chat feature', '--description', 'From the chat', '--yes'], GL);
    ok(r, 'glab mr create');
    expect(gitlab.prs('o/r').length === 1, `the forge has one merge request; glab said: ${r.stdout.trim().slice(0, 200)}`);
  });
  await rp.step('glab mr list and glab mr view (gitlab.com)', async () => {
    const list = await chatRun('glab', ['mr', 'list'], GL);
    ok(list, 'glab mr list');
    expect(list.stdout.includes('Chat feature'), `the list names the merge request: ${list.stdout}`);
    const view = await chatRun('glab', ['mr', 'view', '1'], GL);
    ok(view, 'glab mr view 1');
    expect(view.stdout.includes('Chat feature'), `the view names the merge request: ${view.stdout}`);
  });
  await rp.step('glab api on the project (gitlab.com)', async () => {
    const r = await chatRun('glab', ['api', 'projects/o%2Fr'], GL);
    ok(r, 'glab api projects/o%2Fr');
    expect(JSON.parse(r.stdout).path_with_namespace === 'o/r', `the project answers: ${r.stdout.slice(0, 120)}`);
  });
  await rp.step('git fetch, git pull and a second push to the chat branch (gitlab.com)', async () => {
    gitlab.teammatePush('o/r', 'main', 'teammate.txt');
    ok(await chatRun('git', ['fetch', 'origin'], GL), 'git fetch');
    ok(await sh(GL, 'git checkout -q main'), 'checkout main');
    ok(await chatRun('git', ['pull', '--ff-only', 'origin', 'main'], GL), 'git pull');
    expect(fs.existsSync(path.join(GL, 'teammate.txt')), 'the pull brought the teammate\'s file');
    ok(await sh(GL, 'git checkout -q chat/feature && echo two >> chat.txt && git commit -q -am "chat edits the file"'), 'second commit');
    ok(await chatRun('git', ['push', 'origin', 'chat/feature'], GL), 'a second push to its own branch');
  });

  // ---- refused -----------------------------------------------------------------------------------

  for (const [label, dir, forge] of [['github.com', () => GH, github], ['gitlab.com', () => GL, gitlab]]) {
    await rp.step(`refused: a push to main, a tag, and a branch the chat did not create (${label})`, async () => {
      const before = forge.refSha('o/r', 'refs/heads/main');
      denied(await chatRun('git', ['push', 'origin', '+HEAD:main'], dir()), 'git push +HEAD:main (forced, so git itself does not stop it first)');
      denied(await chatRun('git', ['push', 'origin', 'HEAD:refs/tags/v1'], dir()), 'a tag push');
      denied(await chatRun('git', ['push', 'origin', ':chat/feature'], dir()), 'deleting the chat\'s branch');
      forge.teammatePush('o/r', 'mates-branch', 'mate.txt');
      ok(await chatRun('git', ['fetch', 'origin'], dir()), 'git fetch of a branch the chat did not create');
      ok(await sh(dir(), 'git checkout -q -b tmp origin/mates-branch && echo x > tmp.txt && git add tmp.txt && git commit -q -m "chat edits a teammate\'s branch"'), 'a commit on top of the teammate\'s branch');
      denied(await chatRun('git', ['push', 'origin', 'tmp:mates-branch'], dir()), 'writing to a branch the chat did not create');
      ok(await sh(dir(), 'git checkout -q chat/feature'), 'back to the chat\'s own branch');
      expect(forge.refSha('o/r', 'refs/heads/main') === before, 'main did not move');
      expect(forge.refSha('o/r', 'refs/tags/v1') === null, 'no tag was made');
      expect(forge.refSha('o/r', 'refs/heads/chat/feature') !== null, 'the chat\'s branch is still there');
    });
  }
  await rp.step('refused: another repo (o/other) over gh, glab and git, which the person can reach', async () => {
    denied(await chatRun('gh', ['api', 'repos/o/other'], GH), 'gh api repos/o/other');
    denied(await chatRun('gh', ['pr', 'list', '-R', 'github.localhost/o/other'], GH), 'gh pr list -R o/other');
    denied(await chatRun('gh', ['api', 'search/repositories?q=x'], GH), 'gh api search');
    unreachable(await chatRun('git', ['ls-remote', 'https://github.com/o/other.git'], GH), 'git ls-remote o/other (github.com)');
    denied(await chatRun('glab', ['api', 'projects/o%2Fother'], GL), 'glab api projects/o%2Fother');
    unreachable(await chatRun('git', ['ls-remote', 'https://gitlab.com/o/other.git'], GL), 'git ls-remote o/other (gitlab.com)');
    for (const f of [github, gitlab]) {
      const hits = f.log.filter((e) => /\/o\/other|o%2Fother/.test(e.path) && e.credentials.length > 0);
      expect(hits.length === 0, `${f.kind}: the forge got a credentialed request for o/other: ${hits.map((e) => `${e.method} ${e.path}`).join('; ')}`);
    }
  });
  await rp.step('the anonymous route: a public repo outside the project reads with no credential; a recorded private one asks for the sentinel first', async () => {
    for (const [f, url] of [[github, 'https://github.com/o/public.git'], [gitlab, 'https://gitlab.com/o/public.git']]) {
      ok(await chatRun('git', ['ls-remote', url], GH), `git ls-remote ${url}`);
      const hits = f.log.filter((e) => e.path.startsWith('/o/public.git'));
      expect(hits.length > 0 && hits.every((e) => e.credentials.length === 0 && e.status === 200), `${f.kind}: the public repo should be read with no credential and answered: ${JSON.stringify(hits.map((e) => [e.path, e.status, e.credentials.length]))}`);
    }
    if (app.bridged) {
      // The core refuses a recorded repo's request that has no sentinel, so git is challenged by the listener and the forge is not asked.
      expect(app.audit.some((a) => a.reason === 'no sentinel yet: challenged'), 'a recorded private repo\'s first request is challenged, and git then asks the helper');
    } else {
      const priv = github.log.filter((e) => e.path.startsWith('/o/r.git/info/refs') && e.credentials.length === 0);
      expect(priv.length > 0 && priv.every((e) => e.status === 401), 'a private repo answers an anonymous request with 401, and git then asks the helper');
    }
  });
  await rp.step('gh auth token gives a chat no token: the wrapper refuses, and what gh itself prints is worth nothing', async () => {
    const wrapped = await chatRun('gh', ['auth', 'token'], GH);
    expect(wrapped.code !== 0 && wrapped.stdout.trim() === '', `the wrapper should print nothing: ${wrapped.stdout}`);
    // Past the wrapper, to the real gh: it prints what is in its environment, the sentinel, and no more.
    const realGh = tool('sh', ['-c', 'command -v gh']).trim();
    const bare = await chat.exec(realGh, ['auth', 'token'], GH);
    rp.outputs.push(bare.stdout, bare.stderr);
    const printed = bare.stdout.trim();
    expect(printed === '' || printed === app.sentinel, `gh printed something that is not the sentinel: ${printed.slice(0, 8)}...`);
    if (printed) {
      for (const f of [github, gitlab]) {
        f.mustNotSee.delete(app.sentinel); // this test sends it to the forge on purpose
        const direct = await app.rawForge(f, f.kind === 'github' ? '/user' : '/api/v4/user', printed);
        f.mustNotSee.add(app.sentinel);
        expect(direct.status === 401, `${f.kind}: the printed value must be worthless at the forge, and it answered ${direct.status}`);
      }
      const mark = app.audit.length;
      const other = await chat.exec(realGh, ['api', 'repos/o/other'], GH);
      expect(other.code !== 0 && app.audit.slice(mark).some((a) => a.verdict === 'refuse'), 'the sentinel gh printed buys nothing from the proxy outside the project');
    }
  });

  // ---- merges are held, then done by the person's press ---------------------------------------------

  await rp.step('gh pr merge becomes a held ask; nothing merges until the person approves; then it merges that head (github.com)', async () => {
    const r = await chatRun('gh', ['pr', 'merge', '1', '--merge'], GH);
    const recent = () => JSON.stringify(app.audit.slice(-4).map((a) => [a.method, a.url, a.verdict, a.reason]));
    denied(r, 'gh pr merge');
    expect((r.stderr + r.stdout).includes('holds merges'), `the refusal should say purlis holds merges: ${tail(r)}`+recent());
    expect(github.merges.length === 0 && github.prs('o/r')[0].state === 'open', 'the forge merged nothing');
    const ask = app.asks.find((a) => a.host === 'github.com' && !a.done);
    expect(ask && ask.number === 1, `an ask for pull request 1 should be raised: ${JSON.stringify(app.asks.map((a) => [a.host, a.number]))}`);
    expect(ask.headSha === github.refSha('o/r', 'refs/heads/chat/feature'), 'the ask names the head the forge has, read from the forge');
    const done = await app.approve(ask);
    expect(done.status === 200, `the person's approval should merge: ${done.status} ${done.body}`);
    expect(github.merges.length === 1 && github.merges[0].head === ask.headSha, 'the forge merged exactly the head the person saw');
    const view = await chatRun('gh', ['pr', 'view', '1', '--json', 'state'], GH);
    ok(view, 'gh pr view after the merge');
    expect(view.stdout.includes('MERGED'), `the chat sees the pull request merged: ${view.stdout}`);
  });
  await rp.step('glab mr merge becomes a held ask; nothing merges until the person approves (gitlab.com)', async () => {
    const r = await chatRun('glab', ['mr', 'merge', '1', '--yes'], GL);
    denied(r, 'glab mr merge');
    expect(gitlab.merges.length === 0 && gitlab.prs('o/r')[0].state === 'open', 'the forge merged nothing');
    const ask = app.asks.find((a) => a.host === 'gitlab.com' && !a.done);
    expect(ask && ask.number === 1, `an ask for merge request 1 should be raised: ${tail(r)} ${JSON.stringify(app.audit.slice(-5).map((a) => [a.method, a.url, a.verdict, a.reason]))}`);
    const done = await app.approve(ask);
    expect(done.status === 200, `the person's approval should merge: ${done.status} ${done.body}`);
    expect(gitlab.merges.length === 1, 'the forge merged it');
  });
  await rp.step('the proxy audits what it held and what it refused, never a token', async () => {
    const held = app.audit.filter((a) => a.verdict === 'hold');
    expect(held.length >= 2, `a hold per merge is audited: ${held.length}`);
    expect(app.audit.some((a) => a.verdict === 'refuse' && a.reason), 'a refusal is audited with its reason');
    expect(!JSON.stringify(app.audit).match(/fake-g[hl]-|sentinel-/), 'the audit holds no token and no sentinel');
  });

  // ---- the token renews unattended --------------------------------------------------------------------

  await rp.step('GitHub\'s token needs no renewal (it does not expire); GitLab\'s renews before it expires, unattended', async () => {
    clock.advance(9 * HOUR); // long past GitLab's 2 hours
    ok(await chatRun('gh', ['pr', 'list', '--state', 'all'], GH), 'gh pr list nine hours later');
    ok(await chatRun('git', ['fetch', 'origin'], GH), 'git fetch (github.com) nine hours later');
    ok(await chatRun('glab', ['mr', 'list', '--all'], GL), 'glab mr list after the token expired');
    ok(await chatRun('git', ['fetch', 'origin'], GL), 'git fetch (gitlab.com) after the token expired');
    expect(!github.grants.includes('refresh_token'), 'GitHub was never asked to refresh');
    expect(gitlab.grants.includes('refresh_token'), 'GitLab saw a refresh grant');
    expect(app.refreshes.some((x) => x.startsWith('gitlab.com') && x.endsWith('before expiry')), `renewals: ${app.refreshes.join('; ')}`);
    expect(!app.refreshes.some((x) => x.startsWith('github.com')), 'no GitHub renewal was made');
  });
  await rp.step('GitLab\'s token renews once on a 401 (the forge revoked it), and the rotated refresh token is the one kept', async () => {
    gitlab.revokeAccessTokens();
    ok(await chatRun('glab', ['mr', 'view', '1'], GL), 'glab mr view after a revocation');
    ok(await chatRun('git', ['fetch', 'origin'], GL), 'git fetch (gitlab.com) after a revocation');
    expect(app.refreshes.some((x) => x.startsWith('gitlab.com') && x.endsWith('after a 401')), `renewals: ${app.refreshes.join('; ')}`);
    clock.advance(9 * HOUR); // a third renewal uses the refresh token the second one returned
    ok(await chatRun('glab', ['mr', 'list'], GL), 'glab mr list after a second expiry');
  });

  // ---- the chat never saw a token -----------------------------------------------------------------------

  await rp.step('the chat never saw a token: its environment, its files and every output are scanned', async () => {
    const stored = [...app.keyring.values()].flatMap((k) => [k.access, k.refresh]).filter(Boolean);
    expect(stored.length >= 3 && stored.every((t) => github.issued.has(t) || gitlab.issued.has(t)), 'the keyring holds GitHub\'s access token and GitLab\'s access and refresh tokens, each one the forge issued');
    const secrets = [...new Set([...github.issued, ...gitlab.issued, ...stored])];
    expect(secrets.length >= 4, 'the forges issued tokens to look for');
    const hit = (text) => secrets.find((s) => text.includes(s));
    for (const [k, v] of Object.entries(chat.env)) expect(!hit(String(v)), `the chat's environment variable ${k} holds a token`);
    expect(chat.env.GH_TOKEN === app.sentinel && chat.env.GITLAB_TOKEN === app.sentinel, 'the chat holds the sentinel where a tool looks for its token');
    let scanned = 0;
    for (const file of filesUnder(chat.dir)) {
      scanned += 1;
      expect(!hit(fs.readFileSync(file).toString('latin1')), `a token is in the chat's file ${path.relative(chat.dir, file)}`);
    }
    expect(scanned > 10, `only ${scanned} files were scanned`);
    expect(!hit(rp.outputs.join('\n')), 'a token is in something a command printed');
    for (const f of [github, gitlab]) {
      expect(f.log.every((e) => !e.sawForbidden), `${f.kind}: the chat's sentinel reached the forge`);
      const strangers = f.log.flatMap((e) => e.credentials).filter((c) => !f.issued.has(c) && c !== app.sentinel); // the sentinel is caught by sawForbidden; the test replays it once on purpose
      expect(strangers.length === 0, `${f.kind}: the forge saw a credential it never issued`);
    }
    const inKeyring = [...app.keyring.values()].every((k) => secrets.includes(k.access));
    expect(inKeyring, 'the keyring holds the tokens, and only the keyring does');
  });

  // ---- what is still a stand-in ---------------------------------------------------------------------------

  for (const [name, slice] of [
    ['the forge proxy: listeners, the full allowlist, the push rule (TODO(slice A))', 'slice A'],
    ['the chat wiring: environment, the gh wrapper, the credential helper (TODO(slice B))', 'slice B'],
    ['the credential sources: the OS keyring, renewal (TODO(slice C))', 'slice C'],
    ['the readiness check and the project record (TODO(slice D))', 'slice D'],
    ['held forge acts (TODO(slice F))', 'slice F'],
    ['the sign-in window (TODO(slice G))', 'slice G'],
  ]) rp.stub(name, slice);

  const md = rp.markdown(`One login I end-to-end test on ${PLATFORM}`);
  fs.writeFileSync(path.join(outDir, `results-${PLATFORM}.md`), md);
  fs.writeFileSync(path.join(outDir, `audit-${PLATFORM}.json`), `${JSON.stringify({ audit: app.audit, github: github.log, gitlab: gitlab.log }, null, 2)}\n`);
  console.log(md);
  if (process.env.GITHUB_STEP_SUMMARY) fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, md);
  app.close();
  github.close();
  gitlab.close();
  const failed = rp.failed();
  console.log(failed.length ? `${failed.length} step(s) failed` : 'every step passed');
  process.exit(failed.length ? 1 : 0);
}

main().catch((e) => {
  console.error('the end-to-end harness failed:', e);
  process.exit(2);
});
