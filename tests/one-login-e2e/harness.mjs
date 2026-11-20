// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What the scenario stands on that is not the product: a CA of the test's own, running a client
// with a scrubbed environment, a manual clock, and the step report.

import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';

export const tool = (cmd, args, opts = {}) =>
  execFileSync(cmd, args, { stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8', timeout: 60_000, ...opts });

/** A clock the test moves: the forge's token lifetimes and the app's renewal both read it. */
export function manualClock(start = Date.parse('2026-11-20T09:00:00Z')) {
  let t = start;
  return { now: () => t, advance: (ms) => (t += ms) };
}

/** A CA of this run's own and a leaf for any set of names (for the forges and for the proxy's terminations). */
export function makeCa(dir) {
  const key = path.join(dir, 'ca.key');
  const pem = path.join(dir, 'ca.pem');
  tool('openssl', ['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', key]);
  fs.writeFileSync(
    path.join(dir, 'ca.cnf'),
    '[req]\ndistinguished_name=dn\nx509_extensions=v3\nprompt=no\n[dn]\nCN=purlis end-to-end CA\n' +
      '[v3]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\nsubjectKeyIdentifier=hash\n',
  );
  tool('openssl', ['req', '-x509', '-new', '-key', key, '-config', path.join(dir, 'ca.cnf'), '-days', '2', '-out', pem]);
  const leafKey = path.join(dir, 'leaf.key');
  tool('openssl', ['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', leafKey]);
  const leaves = new Map();
  return {
    pem,
    pemText: fs.readFileSync(pem, 'utf8'),
    leafFor(names) {
      const id = names.join(',');
      if (leaves.has(id)) return leaves.get(id);
      const stem = path.join(dir, `leaf-${leaves.size}`);
      const san = names.map((n) => (net.isIP(n) ? `IP:${n}` : `DNS:${n}`)).join(',');
      fs.writeFileSync(`${stem}.ext`, `subjectAltName=${san}\nextendedKeyUsage=serverAuth\nkeyUsage=digitalSignature\nbasicConstraints=CA:FALSE\n`);
      tool('openssl', ['req', '-new', '-key', leafKey, '-subj', `/CN=${names[0]}`, '-out', `${stem}.csr`]);
      tool('openssl', ['x509', '-req', '-in', `${stem}.csr`, '-CA', pem, '-CAkey', key, '-CAcreateserial', '-days', '2', '-extfile', `${stem}.ext`, '-out', `${stem}.pem`]);
      const leaf = { key: fs.readFileSync(leafKey), cert: fs.readFileSync(`${stem}.pem`) };
      leaves.set(id, leaf);
      return leaf;
    },
  };
}

/** Run a program, never throwing: `{ code, stdout, stderr }`. */
export function run(cmd, args, { env, cwd, timeout = 60_000 }) {
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

/** The report: each step passes or fails with a reason; the run fails if any step does. */
export function reporter() {
  const rows = [];
  const outputs = []; // every command's output, for the final scan for secrets
  return {
    rows,
    outputs,
    /** Run one step; a thrown error is the step's failure and the run goes on to the next step. */
    async step(name, fn) {
      process.stdout.write(`step   ${name}\n`);
      try {
        const detail = await fn();
        rows.push({ name, ok: true, detail: detail ?? '' });
        console.log(`pass   ${name}`);
      } catch (e) {
        rows.push({ name, ok: false, detail: String(e.message ?? e) });
        console.log(`FAIL   ${name}\n         ${String(e.message ?? e).split('\n').join('\n         ')}`);
      }
    },
    /** A seam of a slice that has not merged: reported, never silent. */
    stub(name, slice) {
      rows.push({ name, ok: true, stub: slice, detail: `stand-in until ${slice} merges` });
    },
    markdown(title) {
      const lines = [`## ${title}`, '', '| step | result | detail |', '|---|---|---|'];
      for (const r of rows) {
        lines.push(`| ${r.name} | ${r.stub ? `stand-in (${r.stub})` : r.ok ? 'pass' : '**FAIL**'} | ${String(r.detail).replaceAll('|', '/').replaceAll('\n', ' ').slice(0, 400)} |`);
      }
      return `${lines.join('\n')}\n`;
    },
    failed: () => rows.filter((r) => !r.ok),
  };
}

export const expect = (cond, message) => {
  if (!cond) throw new Error(message);
};

/** The last lines of a command's output, for a failure message. */
export const tail = (r) => `exit ${r.code}: ${(r.stderr || r.stdout).trim().split('\n').slice(-4).join(' | ').slice(0, 500)}`;

/** Every file under a directory, with its text, for the scan for secrets. */
export function filesUnder(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name === 'objects' && dir.endsWith('.git')) continue; // packed history is not config
      filesUnder(p, out);
    } else if (e.isFile() && fs.statSync(p).size < 5_000_000) out.push(p);
  }
  return out;
}
