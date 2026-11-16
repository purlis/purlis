// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `CONTEXT.md`'s words for "Any model on any harness" that phase 1a makes true (T1a-19, #6767):
// each entry lands only the part 1a makes true, word for word from the design's §2.2 and §2.3, and
// the clauses built in 1b, 1c, 2 and 3 land with their phases (U22, C-11, C-12). An entry is
// compared whole, its words as written and its line breaks ignored, so a later phase that widens
// one changes it here in the same pull request.
//
// **Block** is not in the list: T1a-26 changes its wording and asserts it. CI runs this through
// `tools/context-terms.test.mjs`, which checks the repository's own `CONTEXT.md`;
// `node tools/context-terms.mjs` runs it by hand.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The entries 1a lands or replaces, each as `CONTEXT.md` must read it. */
export const TERMS = {
  // R-008, design §2.2, whole.
  Route: `**Route** (of a run):
What one run's model calls are bound to at its start: a **profile**, a **model provider
declaration**, exactly as it stood at the start, and the endpoint whose format is the harness's.
The run keeps it to its end; another route is another run (ADR 0087, amended for #6578).
_Avoid_: route (unqualified, for a way of reaching a model; that is the sign-in kind), path,
connection`,

  // R-009, design §2.2, whole: always set against the egress proxy.
  "Key proxy": `**Key proxy**:
The listener in \`purlisd\` that a chat's harness sends its model calls to, one per chat: it
checks the run's token, refuses what its contract doesn't allow, puts the credential in, and
passes every body through unchanged. On a runner it is the runner host's own (ADR 0087 §7,
amended for #6578).
_Avoid_: proxy (unqualified; the egress proxy is ADR 0067's per-chat proxy for a sandboxed
chat's connections), gateway, passthrough`,

  // R-011, design §2.2, its 1a part: keychain values only, model keys only.
  "Your keys": `**Your keys**:
The one list on this machine of the person's model keys: each a value in the system keychain. It
is the only place a model key is read from, and only the **key proxy** uses it, for any project or
persona. It is not a **Vault**: it has no name, no persona grant and no reference a project can
write, and no chat, command or extension is handed anything from it. Each key carries a mark that
binds it to one **model provider declaration** and its origins; only the window marks a key, and a
key written any other way loses its mark. The window shows it as *Your keys*, under You › Models
(ADR 0087, amended for #6578).
_Avoid_: vault (for it; that is a **Vault**), key store, keyring (that is where its values are kept)`,

  // R-014, design §2.3, its 1a part: the wire formats a built-in speaks, and no more.
  "Harness declaration": `**Harness declaration**:
Data that says how to start one harness, how to name and resume its sessions, which levels it
offers, what it can do, and what its profiles are: the **sign-in kinds** they support, the
folder variables, whether their credentials can be kept from the harness's commands, and how a
conversation moves. The ones purlis ships with adapter code (Claude Code, Codex and opencode,
and those that join them) also say which wire formats the harness speaks; how one launch is
routed is the adapter's. A project may declare more, which each machine approves before they
run, which never replace a built-in's, and which run on their own sign-in only (ADR 0073,
amended for #6578).
_Avoid_: harness definition, harness config (that is the harness's own), profile (that is one
identity of the harness the declaration describes)`,

  // R-018, design §2.3, its 1a part: a chat on a key.
  "Internet access": `**Internet access**:
Which hosts a sandboxed chat can reach: readable presets ("Model providers", "Code hosting",
"Package registries"), each listing its hosts, plus the project's own hosts and your personal
ones. The project's choice is committed and every teammate follows it; a host nothing allows is
blocked and shown. A chat on a key needs no host here: its model calls go through the **key
proxy** (ADR 0067 §1, §3, amended for #6578).
_Avoid_: hosts it may reach, egress (in UI text), allowlist (that is the commit scan's), network
policy`,

  // U22: the per-pool answer, pressed as W124.
  "Pool key allowance": `**Pool key allowance**:
The person's answer that lets one key of **your keys** be sent to one **pool**'s chats, pressed as
*Send key to pool <name>*, and taken back with *Stop sending* in Your keys. Each chat there is
sent the key for its run, and only to a runner marked as the person's own (ADR 0078, amended for
#6578).
_Avoid_: grant (that is a runner's **Grant**), approval (that is a key's approval setting),
allowance (unqualified)`,
};

/** U22: the sentence **Ask** gains, for an Ask keyed by what is missing. */
export const ASK_SENTENCE =
  "An ask may be keyed by what is missing, a key or a **pool key allowance**, not by a chat, and may be raised after the turn ends.";

/**
 * U22: what each entry must not say in 1a: a later phase's clause, or what 1a makes false. Each
 * is matched without regard to line breaks or case.
 */
export const NOT_IN_1A = {
  "Your keys": ["1Password", "HashiCorp Vault", "cloud access keys"],
  "Internet access": ["a cloud or a local model server", "WebFetch", '"AI providers"'],
  "Harness declaration": ["minimum context", "subscription models"],
  // 1a reads a model key from your keys only, and refuses a cloud record (R-651, R-663).
  "Sign-in kind": ["vault"],
  "Model source": ["vault"],
};

/** Words, each separated from the next by one space: line breaks don't count. */
const flat = (text) => text.split(/\s+/).filter(Boolean).join(" ");

/** The first line of an entry: `**Term**:` or `**Term** (qualifier):`. */
const HEAD = /^\*\*[^*]+\*\*(?: \([^)]*\))?:$/;

/** The entry for `term` in `text`, from its head to the line before the next entry or heading. */
export function entry(text, term) {
  const lines = text.split("\n");
  const start = lines.findIndex(
    (line) => HEAD.test(line) && line.startsWith(`**${term}**`),
  );
  if (start === -1) return null;
  let end = start + 1;
  while (end < lines.length && !HEAD.test(lines[end]) && !/^#+ /.test(lines[end])) end += 1;
  return lines.slice(start, end).join("\n").trim();
}

/** The entry's words without its `_Avoid_` line, which names the words it is not. */
const body = (text) => text.split("\n_Avoid_:")[0];

/** R-008, R-009, R-011, R-014, R-018: each entry 1a lands, as it must read. */
export function missing(text) {
  const found = [];
  for (const [term, wanted] of Object.entries(TERMS)) {
    const has = entry(text, term);
    if (has === null) found.push(`CONTEXT.md has no **${term}** entry`);
    else if (flat(has) !== flat(wanted))
      found.push(`CONTEXT.md's **${term}** entry isn't 1a's; it should read:\n${wanted}`);
  }
  return found;
}

/** U22: no entry says what 1a doesn't build, Ask carries its sentence, and R-009's qualifier. */
export function untrue(text) {
  const found = [];
  for (const [term, clauses] of Object.entries(NOT_IN_1A)) {
    const has = entry(text, term);
    if (has === null) {
      found.push(`CONTEXT.md has no **${term}** entry`);
      continue;
    }
    const words = flat(has).toLowerCase();
    for (const clause of clauses)
      if (words.includes(flat(clause).toLowerCase()))
        found.push(`**${term}** says "${clause}", which 1a doesn't make true`);
  }
  const ask = entry(text, "Ask");
  if (ask === null || !flat(ask).includes(ASK_SENTENCE))
    found.push(`**Ask** doesn't say: ${ASK_SENTENCE}`);
  const allowance = entry(text, "Pool key allowance");
  if (allowance !== null && /\b(grant|approval)/i.test(body(allowance)))
    found.push('**Pool key allowance** calls itself a "grant" or an "approval"');
  // R-009: the key proxy is never a bare "proxy" in what this ticket lands.
  for (const term of [...Object.keys(TERMS), "Sign-in kind", "Model source"]) {
    const has = entry(text, term);
    if (has === null) continue;
    for (const hit of flat(body(has)).matchAll(/(\S+) proxy\b/gi))
      if (!/^(key|\*\*key|egress)$/i.test(hit[1]))
        found.push(`**${term}** says "${hit[0]}", not "key proxy" or "egress proxy"`);
  }
  return found;
}

const CONTEXT = fileURLToPath(new URL("../CONTEXT.md", import.meta.url));

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const text = readFileSync(CONTEXT, "utf8");
  const found = [...missing(text), ...untrue(text)];
  for (const one of found) console.error(one);
  process.exit(found.length === 0 ? 0 : 1);
}
