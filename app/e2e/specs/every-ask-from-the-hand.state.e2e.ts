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
  HOST_KEPT_BLOCKED,
  INBOX_DENIED,
  REACHED_HOST,
  REACHES_A_HOST,
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
import { endEveryChat, pressAndStart } from "../opening.js";
import { PROJECTS, ask, inFront, keepTheNotices, openFromTheStrip } from "../switching.js";

/**
 * **Every kind of Blocking ask is found from the ✋ alone** (#6001, spec #5997, ADR 0121
 * decisions 2 and 4, Consequences). The specs that shipped ADR 0119 opened their views directly,
 * so none of them checked that a request can be seen from the ✋, and asks went unseen.
 *
 * Here each kind is raised **in another project and another workspace** than the one in front:
 * projects of the spec's own, opened beside the launch's, their chats started in their `beta`
 * workspace. One keeps its sandbox at the floor, as the run's own project does; the other has it
 * in force (`[sandbox] mode = "on"`), which the sandbox consent and the host block need. The launch's project is then brought to the front, and from there, with **no view
 * opened by the test**, each kind must be:
 *
 * - **counted** by the ✋, as "+N elsewhere", and that N is what Asks lists for the other
 *   projects, and the asking project's line lists every one of its asks;
 * - **listed** by Asks, which opens on all projects, under that project's line, saying what it
 *   asks;
 * - **reachable**: Go to project brings the asking project forward with Asks on the ask's row,
 *   titled as its kind is, and a decision's options stand on that row (ADR 0121 amended
 *   2026-11-09). Another project's asks are read in the project in front and answered in their
 *   own (`InboxElsewhere.tsx`: "Read here and answered there").
 *
 * Each is then answered with its refusal, or the cheapest safe answer, and **the asker is
 * told**: the chat's pane, or the command's own ending, says so. Each kind is a test of its own,
 * so a failure names its kind.
 *
 * **Nothing is faked past the harness.** The reporting harness (`wdio.state.conf.ts`) raises
 * each chat's ask through the real `charter` (`harness.ts`): a permission through its hook, a
 * question by ending its turn, a secret with `secret ask`, a run grant with `secret exec`, a
 * dispatch with `dispatch`, a host through the chat's own sandbox proxy. The sandbox consent is
 * the project's own committed `expects`, and the command confirmation is the real `purlis
 * signing check`, run as the person runs it, in a terminal of no chat (it refuses to run in one).
 *
 * **Not raised here: a browser site.** A chat's browser holds a site back inside the real
 * browser engine (`browsers/judge.rs`), which this run neither has nor sets up; only the browser
 * run does, and no spec drives a chat's browser yet. It is listed as pending below.
 *
 * **It leaves the window as it found it**: every trigger file is removed, every chat it started
 * is ended, its project is closed, and the launch's project is in front.
 */

/** The title bar's hand. */
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';
/** The Asks view, wherever the right side's bar draws it. */
const INBOX = '[role="tabpanel"][data-view="asks"]';
/** The right bar's ✋: its count is in its name, after "Asks". */
const ASKS_TAB = '[role="tablist"][aria-label="Attention"] [role="tab"][aria-label="Asks"]';
/** The lines for this window's other projects. */
const OTHERS = `${INBOX} [role="rowgroup"][data-others]`;

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
/** **The project with its sandbox in force** (`[sandbox] mode = "on"`): a host it has not
 *  allowed is asked about, and so is a service it expects that this machine has not allowed. */
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
const TRIGGERS = [
  ASKS_A_PERMISSION,
  ASKS_A_SECRET,
  RUNS_A_SECRET,
  DISPATCHES_A_TASK,
  REACHES_A_HOST,
];

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

/** What Asks says of the window's other projects, and what the ✋ counts elsewhere; one pass. */
async function elsewhere(): Promise<{
  counted: number | null;
  lines: { go: string; count: number; says: string[] }[];
}> {
  return browser.execute(
    (tab: string, others: string) => {
      const [, badge] = (document.querySelector(tab)?.getAttribute("aria-labelledby") ?? "").split(
        /\s+/,
      );
      const said = badge ? (document.getElementById(badge)?.textContent ?? "") : "";
      const counted = /\+(\d+) elsewhere/.exec(said);
      return {
        counted: counted ? Number(counted[1]) : null,
        lines: [
          ...document.querySelectorAll<HTMLElement>(`${others} > [role="row"][data-key]`),
        ].map((line) => ({
          go:
            line.querySelector('[aria-label^="Go to project "]')?.getAttribute("aria-label") ?? "",
          count: Number(
            / · (\d+) waiting on you$/.exec(
              line.querySelector(":scope .ask-says")?.textContent?.trim() ?? "",
            )?.[1] ?? Number.NaN,
          ),
          says: [...line.querySelectorAll<HTMLElement>("li.inbox-says .ask-says")].map(
            (one) => one.textContent?.trim() ?? "",
          ),
        })),
      };
    },
    ASKS_TAB,
    OTHERS,
  );
}

/** Asks's own words, for a failure to say what it showed. */
async function asksSaid(): Promise<string> {
  return browser.execute(
    (inbox: string) =>
      document.querySelector<HTMLElement>(inbox)?.innerText.replace(/\s+/g, " ").trim() ??
      "(no Asks)",
    INBOX,
  );
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
  /** Answers it on its row in the asking project's Asks, with the row in hand. */
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

/** Presses the row's `label`: a grant's settle is waited out by `pressGrant`, a refusal too. */
async function pressOnRow(row: string, label: string): Promise<void> {
  await pressGrant($(row).$(`button=${label}`));
}

/** Keep blocked: its first press opens the optional "Why keep it blocked?" box, its second
 *  sends the refusal (ADR 0121 amended 2026-11-09). */
async function keepBlocked(row: string): Promise<void> {
  const keep = $(row).$("button=Keep blocked");
  await pressGrant(keep);
  if (await keep.isExisting()) await pressGrant(keep);
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

/** Presses `label` on `row` from the page: the confirmation's own dialog is up over the window,
 *  as it is for the person, so nothing under it takes the driver's click. */
async function pressFromThePage(row: string, label: string): Promise<void> {
  const pressed = await browser.execute(
    (at: string, words: string) => {
      const button = [...document.querySelectorAll<HTMLElement>(`${at} button`)].find(
        (one) => one.textContent?.trim() === words,
      );
      button?.click();
      return button !== undefined;
    },
    row,
    label,
  );
  expect(pressed).toBe(true);
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
    name: "sandbox host block",
    from: walled,
    source: "sandbox-host",
    says: new RegExp(
      `^(A connection to ${literally(REACHED_HOST)} waits on your answer|The sandbox refused ${literally(REACHED_HOST)})$`,
    ),
    decision: {
      title: "Allow reaching this host?",
      allow: "Allow only for this chat",
      refuse: "Keep blocked",
    },
    raise: () => fromAChat(walled, REACHES_A_HOST),
    answer: keepBlocked,
    told: () => panesTell("sandbox host block", HOST_KEPT_BLOCKED),
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
      await toFront(front);
    },
    answer: (row) => pressFromThePage(row, "Cancel"),
    told: async () => {
      const ended = await signing?.ended;
      expect(ended).toBe(1);
      expect(signing?.said).toContain("it was declined in the window");
    },
  },
];

describe("every Blocking ask, found from the ✋ alone (#6001)", function () {
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
    // A dialog a failed test left up (a picker, a confirmation) is put away first, so the next
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
    it(`finds and answers a ${kind.name} from the ✋ alone`, async () => {
      await kind.raise();
      expect(await inFront()).toBe(front);

      // **From the ✋ alone**: the hand is the one thing pressed, and only if Asks is not open.
      if (!(await $(INBOX).isDisplayed())) await $(HAND).click();
      await $(INBOX)
        .waitForDisplayed({ timeout: 20_000 })
        .catch(() => {
          throw new Error(`${kind.name}: the ✋ did not open Asks`);
        });

      // **Counted and listed**: the asking project's line says it, the line counts all it lists,
      // and the ✋ counts, elsewhere, what every other project's line counts.
      let seen = await elsewhere();
      const ours = () => seen.lines.find((line) => line.go === `Go to project ${kind.from.name}`);
      await browser
        .waitUntil(
          async () => {
            seen = await elsewhere();
            const line = ours();
            return (
              line !== undefined &&
              line.says.some((said) => kind.says.test(said)) &&
              line.count === line.says.length &&
              seen.counted === seen.lines.reduce((all, one) => all + one.count, 0)
            );
          },
          { timeout: 90_000, interval: 500 },
        )
        .catch(async () => {
          const kept = await keepTheNotices(`every-ask-${kind.source}`);
          throw new Error(
            `${kind.name}: not counted or not listed from the ✋. The ✋ counts ${seen.counted} elsewhere; ` +
              `the lines say ${JSON.stringify(seen.lines)}; Asks says: ${await asksSaid()} (kept: ${kept})`,
          );
        });

      // **Reachable**: Go to project brings the asking project forward, Asks on the row.
      await browser.execute(
        (others: string, name: string) =>
          document
            .querySelector<HTMLElement>(`${others} [aria-label="Go to project ${name}"]`)
            ?.click(),
        OTHERS,
        kind.from.name,
      );
      await browser.waitUntil(async () => (await inFront()) === kind.from.root, {
        timeout: 20_000,
        timeoutMsg: `${kind.name}: Go to project did not bring the asking project forward`,
      });
      const row = `${INBOX} .inbox-ask[data-source="${kind.source}"]`;
      await $(row)
        .waitForDisplayed({ timeout: 20_000 })
        .catch(async () => {
          throw new Error(
            `${kind.name}: the asking project's Asks has no row for it; it says: ${await asksSaid()}`,
          );
        });
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

      await kind.answer(row);
      await kind.told();
    });
  }

  // A chat's browser holds a site back inside the real browser engine, which this run has not:
  // pending until a spec of the browser run drives a chat's browser.
  it("finds and answers a browser site from the ✋ alone (needs the browser run's engine)");
});
