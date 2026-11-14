// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ChildProcess, spawn } from "node:child_process";
import {
  appendFileSync,
  copyFileSync,
  mkdirSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import { focusWorkspace, pressGrant } from "../acting.js";
import {
  ASKED_KEY,
  ASKS_A_PERMISSION,
  ASKS_A_SECRET,
  DISPATCHED_TO,
  DISPATCHES_A_TASK,
  DISPATCH_KEPT_BLOCKED,
  INBOX_DENIED,
  RUNS_A_SECRET,
  SECRET_ASKED,
  SECRET_KEY,
  SECRET_RAN,
  SECRET_VAULT,
  THE_RUNS_HOME,
  THE_RUNS_TREE,
  WAITING_ON_THE_PERSON,
  anEmptyRecord,
  built,
  copyFixturePlane,
} from "../harness.js";
import { endEveryChat, pressAndStart, showView } from "../opening.js";
import { PROJECTS, ask, inFront, keepTheNotices, openFromTheStrip } from "../switching.js";

/**
 * **Every kind of Blocking ask is found from its project's tab and that project's ✋** (#6001,
 * spec #5997, ADR 0121 decisions 2 and 4, Consequences, as amended 2026-11-12). The specs that
 * shipped ADR 0119 opened their views directly, so none of them checked that a request can be
 * seen from where it is counted, and asks went unseen. (The file keeps its name from when the ✋
 * counted every project.)
 *
 * Here each kind is raised **in another project and another workspace** than the one in front:
 * projects of the spec's own, opened beside the launch's, their chats started in their `beta`
 * workspace. One keeps its sandbox at the floor, as the run's own project does, and its chats ask;
 * the other has it in force (`[sandbox] mode = "on"`), which its consent needs. The launch's
 * project is then brought to the front, and from there, opening nothing but Asks, each kind
 * must be:
 *
 * - **counted** on its own project's tab on the rail; the ✋ counts the project in front alone
 *   (#6341, ADR 0121 amended 2026-11-12, decision 1), so it never counts it, and the Asks in
 *   front lists no other project's ask, as a row or a line (#6340, decision 3);
 * - **reached from its tab**: pressed, its project is in front, and that project's ✋ counts it;
 * - **listed** by that project's Asks, which opens on Chat: in the scope drawn, or one press
 *   wider, on the way out an empty scope offers or on the Project chip (decision 4), as a row of
 *   its own saying what it asks;
 * - **answerable in place**: its row is titled as its kind is, and a decision's options stand on
 *   it (ADR 0121 amended 2026-11-09), or a typed ask keeps its Answer.
 *
 * Each is then answered there, in its own project, with its refusal or the cheapest safe answer,
 * and **the asker is told**: the chat's pane, or the command's own ending, says so. Each kind is a
 * test of its own, so a failure names its kind. Reaching an ask from its notification is held by
 * `AsksEveryProject.window.test.tsx`: the click comes from the desktop, which no spec presses.
 *
 * **Nothing is faked past the harness.** The reporting harness (`wdio.state.conf.ts`) raises
 * each chat's ask through the real `charter` (`harness.ts`): a permission through its hook, a
 * question by ending its turn, a secret with `secret ask`, a run grant with `secret exec`, a
 * dispatch with `dispatch`. The sandbox consent is
 * the project's own committed `expects`, and the command confirmation is the real `purlis
 * signing check`, run as the person runs it, in a terminal of no chat (it refuses to run in one).
 *
 * **Not raised here, and listed as pending below:**
 *
 * - **A browser site.** A chat's browser holds a site back inside the real browser engine
 *   (`browsers/judge.rs`), which this run neither has nor sets up; only the browser run does,
 *   and no spec drives a chat's browser yet.
 * - **A sandbox host block.** At the floor, where this run's chats start, the network is open
 *   and no host is asked about. Where the sandbox is in force, purlis starts no chat whose
 *   program lives where a chat may write, and this run's harness lives in its temporary folder.
 *
 * **It leaves the window as it found it**: every trigger file is removed, every chat it started
 * is ended, its projects are closed, and the launch's project is in front.
 */

/** The Asks view, wherever the right side's bar draws it. */
const INBOX = '[role="tabpanel"][data-view="asks"]';
/** The right bar's ✋: its count is in its name, after "Asks". */
const ASKS_TAB = '[role="tablist"][aria-label="Attention"] [role="tab"][aria-label="Asks"]';
/** The rows of the asks in an Asks view. */
const ROWS = `${INBOX} .inbox-ask[role="row"]`;
/** Any row or line of another project's asks, which no Asks draws since #6340. */
const ELSEWHERE = `${INBOX} [data-elsewhere], ${INBOX} [role="rowgroup"][data-others]`;
/** Asks's scope chips (ADR 0121, amended 2026-11-12, decision 2). */
const CHIPS = `${INBOX} [role="radiogroup"][aria-label="Show what needs you in"]`;

/** One of the spec's own projects: its folder, which is the name Asks gives it. */
type Project = { root: string; name: string };

/** A project of the spec's own, named `name`: the fixture, renamed, with no record to put back. */
function aProject(name: string): Project {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), name);
  renameSync(copied, renamed);
  const root = realpathSync(renamed);
  anEmptyRecord(root);
  return { root, name };
}

/** The project most kinds are asked from: its sandbox at the floor, as the run's own. */
const mine = aProject("every-ask");
/** **The project with its sandbox in force** (`[sandbox] mode = "on"`): a service it expects
 *  that this machine has not allowed is asked about. */
const walled = aProject("every-ask-walled");

/** The service the project expects, a reverse-DNS name no machine runs. */
const EXPECTED = "com.example.purlis-e2e.every-ask";
/** The value the persona's vault holds already, so the run grant asks only about `curl`. */
const VALUE = "page-e2e-6001-5a2c";
/** What the person replies to the chat that waits on a reply. */
const REPLY = "not now, thanks";

/** `text`, read by a regular expression as itself. */
const literally = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Every trigger file this spec writes. */
const TRIGGERS = [ASKS_A_PERMISSION, ASKS_A_SECRET, RUNS_A_SECRET, DISPATCHES_A_TASK];

/** What every pane of the window shows, one pass. */
async function panesSay(): Promise<string> {
  return browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>('[data-testid="pane"] .xterm-rows')]
      .map((rows) => rows.innerText)
      .join("\n"),
  );
}

/** Brings `plane` to the front from its tab on the projects strip, pressed from the page. */
async function toFront(plane: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if ((await inFront()) === plane) return true;
      await browser.execute(
        (strip: string, path: string) =>
          [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
            .find((tab) => tab.getAttribute("data-directory") === path)
            ?.click(),
        PROJECTS,
        plane,
      );
      return false;
    },
    { timeout: 20_000, interval: 250, timeoutMsg: `${plane} never came to the front` },
  );
}

/** What each of `roots`' tabs on the rail counts (null where it has no tab), and how many rows or
 *  lines of another project's asks are drawn anywhere in sight (#6340, #6341); one pass. */
async function counted(roots: readonly string[]): Promise<{
  tabs: (number | null)[];
  elsewhere: number;
}> {
  return browser.execute(
    (strip: string, elsewhere: string, all: string[]) => {
      // Each project's tab says its count in its description: "1 waiting".
      const tabSays = (root: string) => {
        const tab = [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].find(
          (one) => one.getAttribute("data-directory") === root,
        );
        if (tab === undefined) return null;
        const said = (tab.getAttribute("aria-describedby") ?? "")
          .split(/\s+/)
          .map((id) => document.getElementById(id)?.textContent ?? "")
          .join(" ");
        return Number(/\b(\d+) waiting\b/.exec(said)?.[1] ?? 0);
      };
      return {
        tabs: all.map(tabSays),
        elsewhere: [...document.querySelectorAll<HTMLElement>(elsewhere)].filter(
          (one) => one.getClientRects().length > 0,
        ).length,
      };
    },
    PROJECTS,
    ELSEWHERE,
    [...roots],
  );
}

/** How many asks `root`'s tab on the rail counts (#6341), from its description ("1 waiting");
 *  `null` where the rail has no tab for it. */
async function tabCounts(root: string): Promise<number | null> {
  return browser.execute(
    (strip: string, path: string) => {
      const tab = [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].find(
        (one) => one.getAttribute("data-directory") === path,
      );
      if (tab === undefined) return null;
      const said = (tab.getAttribute("aria-describedby") ?? "")
        .split(/\s+/)
        .map((id) => document.getElementById(id)?.textContent ?? "")
        .join(" ");
      return Number(/\b(\d+) waiting\b/.exec(said)?.[1] ?? 0);
    },
    PROJECTS,
    root,
  );
}

/** What the rows of the Asks in sight say, and whether one of them is a `source`; one pass. */
async function rowsSay(source: string): Promise<{ says: string[]; texts: string[] }> {
  return browser.execute(
    (inbox: string, rows: string, of: string) => {
      const inSight = [...document.querySelectorAll<HTMLElement>(inbox)].find(
        (one) => one.offsetParent !== null,
      );
      const listed = [
        ...(inSight?.querySelectorAll<HTMLElement>(`${rows}[data-source="${of}"]`) ?? []),
      ];
      return {
        says: listed.map((row) => row.querySelector(".ask-says")?.textContent?.trim() ?? ""),
        texts: listed.map((row) => row.innerText.replace(/\s+/g, " ").trim()),
      };
    },
    INBOX,
    ROWS,
    source,
  );
}

/**
 * **One press wider** (decision 4): the way out an empty scope offers ("1 waiting in this
 * project"), else the Project chip. Pressed from the page; nothing where neither is drawn.
 */
async function widen(): Promise<void> {
  await browser.execute(
    (inbox: string, chips: string) => {
      const inSight = [...document.querySelectorAll<HTMLElement>(inbox)].find(
        (one) => one.offsetParent !== null,
      );
      const wayOut = [
        ...(inSight?.querySelectorAll<HTMLElement>(".inbox-scope-empty button") ?? []),
      ].find((one) => /waiting in this project$/.test(one.textContent?.trim() ?? ""));
      if (wayOut !== undefined) {
        wayOut.click();
        return;
      }
      [
        ...(inSight?.querySelectorAll<HTMLElement>(
          `${chips} [role="radio"], ${chips} input[type="radio"]`,
        ) ?? []),
      ]
        .find((one) => (one.getAttribute("aria-label") ?? "").startsWith("Project"))
        ?.click();
    },
    INBOX,
    CHIPS,
  );
}

/** Asks's own words, for a failure to say what it showed. */
async function asksSaid(): Promise<string> {
  return browser.execute((inbox: string) => {
    // Every project's view keeps its Asks drawn behind the one in front: each is said, and
    // whether it is the one in sight.
    const all = [...document.querySelectorAll<HTMLElement>(inbox)];
    if (all.length === 0) return "(no Asks)";
    return all
      .map(
        (one) =>
          `[${one.offsetParent === null ? "hidden" : "in sight"}] ${(one.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 400)}`,
      )
      .join(" | ");
  }, INBOX);
}

/** One kind of Blocking ask, as this spec raises, finds and answers it. */
type Kind = {
  /** Its name, as a failure says it. */
  name: string;
  /** The project it is asked from. */
  from: Project;
  /** Its row's `data-source`. */
  source: string;
  /** What its line says, in the asking project's list and on its row. */
  says: RegExp;
  /** A decision's title, its question, with the options standing on its row. */
  decision?: { title: string; allow: string; refuse: string };
  /** What a row that is not a decision says of where it waits. */
  waits?: string;
  /** Raises it. */
  raise: () => Promise<void>;
  /** Answers it on its row in its own project's Asks, with the row in hand. */
  answer: (row: string) => Promise<void>;
  /** Waits until the asker was told; throws, saying what it saw, if it never was. */
  told: () => Promise<void>;
};

/** The project in front when the spec began: the launch's. */
let front = "";

/** **Starts a chat in `project`'s `beta` workspace** with `trigger` in place, types the line its
 *  harness waits for, and brings the launch's project back to the front. */
async function fromAChat(project: Project, trigger?: string): Promise<void> {
  await toFront(project.root);
  await focusWorkspace("beta");
  if (trigger) writeFileSync(trigger, "");
  await pressAndStart("New tab").catch(async (error: unknown) => {
    // What stood in the start's way, for the failure to say.
    const picker = await browser.execute(
      () =>
        document.querySelector<HTMLElement>('[role="dialog"]')?.innerText.replace(/\s+/g, " ") ??
        "(no dialog)",
    );
    const kept = await keepTheNotices(`every-ask-start-${project.name}`);
    throw new Error(
      `no chat started in ${project.name}: ${String(error)}; the dialog says: ${picker} (kept: ${kept})`,
    );
  });
  const pane = await $('[data-testid="pane"]');
  await pane.click();
  await $(".xterm-helper-textarea").addValue("\n");
  await toFront(front);
}

/** Waits until the panes say `words`, else throws naming `kind`. */
async function panesTell(kind: string, words: string | RegExp): Promise<void> {
  await browser
    .waitUntil(
      async () => {
        const said = await panesSay();
        return typeof words === "string" ? said.includes(words) : words.test(said);
      },
      { timeout: 150_000, interval: 500 },
    )
    .catch(async () => {
      throw new Error(
        `${kind}: the asker was never told; its pane says: ${(await panesSay()).slice(-800)}`,
      );
    });
}

/** **Waits for `row` to stand**, and says what Asks shows where it never does. */
async function rowNow(row: string): Promise<void> {
  await $(row)
    .waitForExist({ timeout: 20_000, interval: 250 })
    .catch(async () => {
      throw new Error(
        `the row ${row} went before it was answered, with ${await inFront()} in front; ` +
          `Asks says: ${await asksSaid()}`,
      );
    });
}

/** Presses the row's `label`: a grant's settle is waited out by `pressGrant`, a refusal too. */
async function pressOnRow(row: string, label: string): Promise<void> {
  await rowNow(row);
  await pressGrant($(row).$(`button=${label}`));
}

/** Keep blocked: its first press opens the optional "Why keep it blocked?" box, its second
 *  sends the refusal (ADR 0121 amended 2026-11-09). */
async function keepBlocked(row: string): Promise<void> {
  // Looked for afresh at each press: the first redraws the row.
  await rowNow(row);
  await pressGrant($(row).$("button=Keep blocked"));
  // Where the first press sent the refusal already, the row goes with it, perhaps while the second
  // is looked for: then there is nothing to press, and `told` says whether the chat heard it.
  if (!(await $(row).isExisting())) return;
  const again = $(row).$("button=Keep blocked");
  if (!(await again.isExisting().catch(() => false))) return;
  await pressGrant(again).catch(async (error: unknown) => {
    if (await $(row).isExisting()) throw error;
  });
}

/** The command confirmation's `purlis signing check`, while it runs. */
let signing: { child: ChildProcess; said: string; ended: Promise<number | null> } | null = null;

/**
 * **Runs `purlis signing check` as the person does, in a terminal of no chat**: it refuses to run
 * in a chat. It finds the window by the machine store the app runs on, which the window says
 * where it is (`theme-live-reload.e2e.ts`), and waits on the window's answer.
 */
async function signingCheck(): Promise<void> {
  const themed = await browser.execute(() => {
    const handed = (window as unknown as Record<string, unknown>).__CHARTER_AT_CREATION__ as {
      theme?: { path?: string };
    };
    return handed?.theme?.path ?? "";
  });
  expect(themed).not.toBe("");
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env))
    if (value !== undefined && !/^(PURLIS|CHARTER)_/.test(key)) env[key] = value;
  Object.assign(env, {
    HOME: THE_RUNS_HOME,
    CHARTER_CONFIG_HOME: dirname(dirname(themed)),
    CHARTER_PLANE_FENCE: THE_RUNS_TREE,
  });
  const child = spawn(built("purlis"), ["signing", "check", "acme/svc", "abc1234"], { env });
  const running = { child, said: "", ended: Promise.resolve<number | null>(null) };
  child.stdout?.on("data", (chunk: Buffer) => (running.said += chunk.toString()));
  child.stderr?.on("data", (chunk: Buffer) => (running.said += chunk.toString()));
  running.ended = new Promise((done) => child.on("exit", (code) => done(code)));
  signing = running;
}

const KINDS: Kind[] = [
  {
    name: "sandbox consent",
    from: walled,
    source: "sandbox-expects",
    says: new RegExp(
      `^This project expects more than this machine allows: .*${literally(EXPECTED)}`,
    ),
    decision: {
      title: "Allow what this project expects?",
      allow: "Allow on this machine",
      refuse: "Not now",
    },
    // The project's committed `expects`, written before it was opened: nothing to start.
    raise: async () => {},
    answer: (row) => pressOnRow(row, "Not now"),
    // No chat asked: it is told by its row going, and by the project's count no longer holding it.
    told: async () => {
      await browser.waitUntil(
        async () => !(await $(`${INBOX} .inbox-ask[data-source="sandbox-expects"]`).isExisting()),
        { timeout: 30_000, timeoutMsg: "sandbox consent: Not now never settled it" },
      );
    },
  },
  {
    name: "question",
    from: mine,
    source: "question",
    says: /^Waiting on your reply$/,
    waits: "Waiting on your reply",
    raise: () => fromAChat(mine),
    answer: async (row) => {
      await rowNow(row);
      await $(row).$("button=Answer").click();
      const reply = await $(`${row} [aria-label^="Reply to "]`);
      await reply.waitForDisplayed({ timeout: 10_000 });
      await reply.addValue(REPLY);
      await browser.keys(["Enter"]);
    },
    told: () => panesTell("question", REPLY),
  },
  {
    name: "permission",
    from: mine,
    source: "permission",
    says: /^Run a command: echo asked-in-the-inbox$/,
    decision: { title: "Allow this action?", allow: "Allow", refuse: "Deny" },
    raise: () => fromAChat(mine, ASKS_A_PERMISSION),
    answer: (row) => pressOnRow(row, "Deny"),
    told: () => panesTell("permission", INBOX_DENIED),
  },
  {
    name: "secret",
    from: mine,
    source: "secret",
    says: new RegExp(`\\b${ASKED_KEY}\\b`),
    waits: "Asks you for a secret its vault doesn't hold",
    raise: () => fromAChat(mine, ASKS_A_SECRET),
    // A value is typed, so its form is behind Answer; Not now is its refusal.
    answer: async (row) => {
      await rowNow(row);
      await $(row).$("button=Answer").click();
      const notNow = await $('[data-testid="sa-not-now"]');
      await notNow.waitForDisplayed({ timeout: 10_000 });
      await notNow.click();
    },
    told: async () => {
      await panesTell("secret", new RegExp(`${SECRET_ASKED} \\d+`));
      const said = await panesSay();
      expect(Number(new RegExp(`${SECRET_ASKED} (\\d+)`).exec(said)?.[1])).toBe(1);
      expect(said).toContain(`declined ${SECRET_VAULT}/${ASKED_KEY}`);
    },
  },
  {
    name: "run grant",
    from: mine,
    source: "run-grant",
    says: /curl/,
    decision: {
      title: `Allow curl to use vault ${SECRET_VAULT}?`,
      allow: "Allow once",
      refuse: "Don't allow",
    },
    raise: () => fromAChat(mine, RUNS_A_SECRET),
    answer: (row) => pressOnRow(row, "Don't allow"),
    told: async () => {
      await panesTell("run grant", new RegExp(`${SECRET_RAN} \\d+`));
      const ended = Number(new RegExp(`${SECRET_RAN} (\\d+)`).exec(await panesSay())?.[1]);
      expect(ended).not.toBe(0);
      expect(ended).not.toBe(WAITING_ON_THE_PERSON);
    },
  },
  {
    name: "dispatch grant",
    from: mine,
    source: "dispatch",
    says: new RegExp(`^Wants to hand a task to ${DISPATCHED_TO}$`),
    decision: {
      title: "Allow this dispatch?",
      allow: "Allow for this chat",
      refuse: "Keep blocked",
    },
    raise: () => fromAChat(mine, DISPATCHES_A_TASK),
    answer: keepBlocked,
    told: () => panesTell("dispatch grant", DISPATCH_KEPT_BLOCKED),
  },
  {
    name: "command confirmation",
    from: mine,
    source: "confirmation",
    says: /^Asks to check abc1234 in acme\/svc on GitHub\. Its terminal shows the code \S+\.$/,
    decision: { title: "Confirm what this command asks?", allow: "Check it", refuse: "Cancel" },
    // A signing check names no project: it is filed under the one in front as it asks, so it is
    // asked with the spec's project in front, which is then put behind.
    raise: async () => {
      await toFront(mine.root);
      await signingCheck();
      await browser
        .waitUntil(async () => /shows the code/.test(signing?.said ?? ""), { timeout: 30_000 })
        .catch(() => {
          throw new Error(
            `command confirmation: signing check never asked; it said: ${signing?.said}`,
          );
        });
      // **No dialog in the way** (#6136): it is answered on its row alone, and the ✋ stays in reach.
      expect(await $('[role="alertdialog"], [role="dialog"]').isExisting()).toBe(false);
      await toFront(front);
    },
    answer: (row) => pressOnRow(row, "Cancel"),
    told: async () => {
      const ended = await signing?.ended;
      expect(ended).toBe(1);
      expect(signing?.said).toContain("it was declined in the window");
    },
  },
];

describe("every Blocking ask, found from its project's tab (#6001, #6341)", function () {
  // A chat's start, the walk between two projects, and the asker's wait to be told.
  this.timeout(300_000);

  before(async () => {
    front = (await ask<string[]>("open_planes"))[0];
    // One app for the whole run: a chat an earlier spec left open would be counted too.
    await endEveryChat();
    // The run's profile, as its own project has it, in each project of the spec's.
    for (const project of [mine, walled])
      copyFileSync(join(front, "charter.local.toml"), join(project.root, "charter.local.toml"));
    // The walled project's sandbox in force, and what it expects of this machine.
    appendFileSync(
      join(walled.root, "charter.toml"),
      `\n[sandbox]\nmode = "on"\nexpects = [${JSON.stringify(EXPECTED)}]\n`,
    );
    // A persona the default one has no grant to dispatch to.
    mkdirSync(join(mine.root, "personas", DISPATCHED_TO), { recursive: true });
    writeFileSync(
      join(mine.root, "personas", DISPATCHED_TO, "persona.md"),
      `---\nname: ${DISPATCHED_TO}\nrole: Reviewer\ndelegate-when: an e2e task\n---\n\nReviews.\n`,
    );
    // The persona's vault, on this machine alone, holding the run's key and not the asked one.
    mkdirSync(join(mine.root, ".charter", "vaults"), { recursive: true });
    writeFileSync(
      join(mine.root, ".charter", "vaults.json"),
      JSON.stringify({
        vaults: {
          [SECRET_VAULT]: {
            provider: "plain-file",
            persona: SECRET_VAULT,
            config: { file: `.charter/vaults/${SECRET_VAULT}-e2e.json` },
          },
        },
      }),
      { mode: 0o600 },
    );
    writeFileSync(
      join(mine.root, ".charter", "vaults", `${SECRET_VAULT}-e2e.json`),
      JSON.stringify({ [SECRET_KEY]: VALUE }),
      { mode: 0o600 },
    );
    await openFromTheStrip(mine.root);
    await openFromTheStrip(walled.root);
    await toFront(front);
  });

  afterEach(async () => {
    for (const trigger of TRIGGERS) rmSync(trigger, { force: true });
    signing?.child.kill();
    signing = null;
    // A dialog a failed test left up (a picker) is put away first, so the next
    // kind still runs: a hook that fails here skips every test after it.
    for (let up = 0; up < 3 && (await $('[role="dialog"]').isExisting()); up++)
      await browser.keys(["Escape"]);
    for (const project of [mine, walled]) {
      await toFront(project.root);
      await endEveryChat();
    }
    await toFront(front);
  });

  after(async () => {
    for (const trigger of TRIGGERS) rmSync(trigger, { force: true });
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== front))
      await ask("close_plane", { plane });
    await toFront(front);
    await endEveryChat();
  });

  for (const kind of KINDS) {
    it(`finds and answers a ${kind.name} from its project's tab`, async () => {
      await kind.raise();
      expect(await inFront()).toBe(front);

      // **Counted on its own project's tab** (#6341), and listed in no Asks of the project in
      // front (#6340, decision 3), with Asks open there.
      await showView("Asks");
      const roots = [mine.root, walled.root];
      let seen = await counted(roots);
      await browser
        .waitUntil(
          async () => {
            seen = await counted(roots);
            return (seen.tabs[roots.indexOf(kind.from.root)] ?? 0) > 0;
          },
          { timeout: 90_000, interval: 500 },
        )
        .catch(async () => {
          const kept = await keepTheNotices(`every-ask-${kind.source}`);
          throw new Error(
            `${kind.name}: not counted on its project's tab. The tabs count ${JSON.stringify(seen.tabs)}; ` +
              `Asks says: ${await asksSaid()} (kept: ${kept})`,
          );
        });
      expect(seen.elsewhere).toBe(0);
      if (await $(INBOX).isDisplayed()) expect((await rowsSay(kind.source)).says).toEqual([]);

      // **Reached from its tab**: pressed, its project is in front; then its ✋ is the one thing
      // pressed, and only if Asks is not open. The right bar's ✋: the title bar's is drawn only
      // where the project in front has asks, and it may not be drawn yet.
      await toFront(kind.from.root);
      if (!(await $(INBOX).isDisplayed())) await $(ASKS_TAB).click();
      await $(INBOX)
        .waitForDisplayed({ timeout: 20_000 })
        .catch(() => {
          throw new Error(`${kind.name}: the ✋ did not open Asks`);
        });

      // **Listed** in the scope Asks opens on, or one press wider (decision 4): a chat's ask is
      // under Chat where its chat has focus; one no chat holds, under Project alone.
      const listed = async () => {
        const { says, texts } = await rowsSay(kind.source);
        const { waits } = kind;
        return (
          says.some((said) => kind.says.test(said)) ||
          (waits !== undefined && texts.some((text) => text.includes(waits)))
        );
      };
      let widened = false;
      await browser
        .waitUntil(
          async () => {
            if (await listed()) return true;
            if (!widened && ((await tabCounts(kind.from.root)) ?? 0) > 0) {
              await widen();
              widened =
                (await $(`${CHIPS} [aria-label^="Project"]:checked`).isExisting()) ||
                (await $(
                  `${CHIPS} [role="radio"][aria-label^="Project"][aria-checked="true"]`,
                ).isExisting());
            }
            return false;
          },
          { timeout: 90_000, interval: 500 },
        )
        .catch(async () => {
          const kept = await keepTheNotices(`every-ask-${kind.source}`);
          throw new Error(
            `${kind.name}: not listed from the ✋ of ${kind.from.name}; ` +
              `Asks says: ${await asksSaid()} (kept: ${kept})`,
          );
        });

      expect(await inFront()).toBe(kind.from.root);
      // **Answerable in place**: its row, in its own project's Asks.
      const row = `${ROWS}[data-source="${kind.source}"]`;
      await $(row)
        .waitForDisplayed({ timeout: 20_000 })
        .catch(async () => {
          throw new Error(
            `${kind.name}: its project's Asks has no row for it; it says: ${await asksSaid()}`,
          );
        });
      // Saying what it asks.
      const says = (await $(row).$(".ask-says").isExisting())
        ? await $(row).$(".ask-says").getText()
        : "";
      expect(
        kind.says.test(says.trim()) ||
          (kind.waits !== undefined && (await $(row).getText()).includes(kind.waits)),
      ).toBe(true);
      if (kind.decision) {
        await expect($(row).$(".inbox-question")).toHaveText(kind.decision.title);
        // **Its options stand on its row**, nothing behind Answer.
        await expect($(row).$(`button=${kind.decision.allow}`)).toBeExisting();
        await expect($(row).$(`button=${kind.decision.refuse}`)).toBeExisting();
        await expect($(row).$("button=Answer")).not.toBeExisting();
      } else {
        // A row that needs typing keeps its form behind Answer, and says where it waits.
        expect(await $(row).getText()).toContain(kind.waits);
        await expect($(row).$("button=Answer")).toBeExisting();
      }

      // Answered in its own project, whose panes are drawn, so the asker is told there.
      await kind.answer(row);
      await kind.told();
    });
  }

  // A chat's browser holds a site back inside the real browser engine, which this run has not:
  // pending until a spec of the browser run drives a chat's browser.
  it("finds and answers a browser site from its project's tab (needs the browser run's engine)");

  // A host is asked about only where the project's sandbox is in force, and there purlis starts
  // no chat whose program lives where a chat may write: this run's harness lives in the run's
  // own temporary folder. Pending until the run keeps its harness outside it.
  it(
    "finds and answers a sandbox host block from its project's tab (needs a harness outside what a chat may write)",
  );
});
