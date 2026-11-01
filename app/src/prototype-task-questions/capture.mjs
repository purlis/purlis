// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #4551, #4561): draws every state to `screenshots/` and measures the
 * spec's visibility rule and smoothness checks against the running page, into `checks.json`.
 * From app/: node src/prototype-task-questions/capture.mjs [path to the built page]
 */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const page_ = resolve(process.argv[2] ?? resolve(here, "PROTOTYPE-artifact.html"));
const url = (s, more = "") => `${pathToFileURL(page_).href}?s=${s}&bare${more}`;
const ids = [...readFileSync(resolve(here, "model.ts"), "utf8").matchAll(/^    id: "([a-z-]+)",$/gm)].map((m) => m[1]);

const shots = resolve(here, "screenshots");
mkdirSync(shots, { recursive: true });
const browser = await chromium.launch({ executablePath: process.env.TQ_CHROMIUM || undefined, args: (process.env.TQ_ARGS ?? "").split(" ").filter(Boolean) });
const context = await browser.newContext({
  viewport: { width: 1280, height: 800 },
  colorScheme: "dark",
});
const page = await context.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

const results = [];
const check = (name, pass, said) => {
  results.push({ name, pass: Boolean(pass), said: String(said) });
  console.log(`${pass ? "pass" : "FAIL"}  ${name}  (${said})`);
};
const load = async (s, more) => {
  await page.goto(url(s, more));
  await page.waitForSelector(".tq-root");
};
const needs = () => page.locator(".tq-root").getAttribute("data-needs").then(Number);
const hand = async () =>
  (await page.locator('[data-testid="needs-you-button"]').count())
    ? Number((await page.locator('[data-testid="needs-you-button"]').innerText()).trim())
    : 0;
const openInbox = async () => {
  if (await page.locator('[data-testid="inbox-card"], .tq-inbox').count()) return;
  if (await page.locator('[data-testid="needs-you-button"]').count())
    await page.locator('[data-testid="needs-you-button"]').click();
};

if (!process.env.NO_SHOTS) {
  await page.goto(url("quiz-count").replace("&bare", ""));
  await page.waitForSelector(".tq-bar");
  await page.screenshot({ path: resolve(shots, "bar.png") });
  await page.keyboard.press("\\");
  await page.waitForSelector(".tq-list");
  await page.screenshot({ path: resolve(shots, "bar-every-state.png") });
  let n = 0;
  for (const s of ids) {
    await load(s);
    n += 1;
    await page.screenshot({ path: resolve(shots, `${String(n).padStart(2, "0")}-${s}.png`) });
  }
  for (const s of ["quiz-choices", "quiz-count", "strip-prose"]) {
    await load(s, "&place=foot");
    await page.screenshot({ path: resolve(shots, `foot-${s}.png`) });
  }
  await page.emulateMedia({ colorScheme: "light" });
  for (const s of ["quiz-count", "strip-long", "row-needs-you", "inbox-list", "trail-agent"]) {
    await load(s);
    await page.screenshot({ path: resolve(shots, `light-${s}.png`) });
  }
  await page.emulateMedia({ colorScheme: "dark" });
}

/* ---- every state: the numbers agree, and nothing open is stranded ---- */
let agree = 0;
let strandedAt = [];
let wrong = [];
for (const s of ids) {
  if (s === "words") continue;
  await load(s);
  const count = await needs();
  if (count !== (await hand())) wrong.push(s);
  await openInbox();
  const cards = await page.locator('[data-testid="inbox-card"]').count();
  if (cards !== count) strandedAt.push(`${s}:${cards}/${count}`);
  agree += 1;
}
check("✋ equals what waits on the person, in every state", wrong.length === 0, wrong.join() || `${agree} states`);
check("Nothing stranded: each item that waits has its one Inbox card, in every state", strandedAt.length === 0, strandedAt.join() || `${agree} states`);

/* ---- in progress stays on the row ---- */
await load("row-asking");
check(
  "In progress: no ✋, no quiz, no Inbox card, a word on the task's row",
  (await needs()) === 0 &&
    (await page.locator('[data-testid="quiz"]').count()) === 0 &&
    (await page.getByTestId("word-dev7").innerText()).startsWith("asking steward 4") &&
    (await page.getByTestId("total-steward").count()) === 0,
  await page.getByTestId("word-dev7").innerText(),
);

await load("row-asking-open");
check("In progress, the task's own tab can answer at once, and ✋ stays 0", (await page.getByTestId("quiz").count()) === 1 && (await needs()) === 0, "drawn, ✋ 0");

/* ---- one text everywhere ---- */
await load("quiz-choices");
await page.getByTestId("needs-you-button").click();
const said = await page.locator("[data-says]").evaluateAll((all) => all.map((one) => one.dataset.says));
check("The same words in the quiz and the Inbox card", said.length === 2 && said[0] === said[1], `${said.length} surfaces`);
await load("strip-ask");
await page.getByTestId("needs-you-button").click();
const said2 = await page.locator("[data-says]").evaluateAll((all) => all.map((one) => one.dataset.says));
check("The same words in the strip and the Inbox card", said2.length === 2 && said2[0] === said2[1], `${said2.length} surfaces`);

/* ---- focus ---- */
for (const place of ["band", "foot"]) {
  await load("quiz-focus", `&place=${place}`);
  const input = page.getByTestId("terminal-input");
  await input.click();
  await page.keyboard.type("pnpm te");
  const before = await input.boundingBox();
  const rowsBefore = (await page.getByTestId("terminal").boundingBox()).height;
  await page.evaluate(() => window.tq.pretend(0));
  await page.getByTestId("quiz").waitFor();
  await page.keyboard.type("st");
  const after = await input.boundingBox();
  const rowsAfter = (await page.getByTestId("terminal").boundingBox()).height;
  const focused = await page.evaluate(() => document.activeElement?.dataset.testid);
  check(
    `A quiz arriving never takes focus, and typing carries on (${place})`,
    focused === "terminal-input" && (await input.inputValue()) === "pnpm test",
    `value "${await input.inputValue()}"`,
  );
  check(
    `The input line stays where it was as a quiz arrives (${place})`,
    Math.abs(after.y - before.y) < 1,
    `moved ${Math.round(after.y - before.y)}px; terminal gave up ${Math.round(rowsBefore - rowsAfter)}px`,
  );
}

/* ---- the keyboard reaches a quiz ---- */
await load("quiz-choices");
await page.getByTestId("terminal-input").click();
await page.keyboard.press("F6");
const inQuiz = await page.evaluate(() => Boolean(document.activeElement?.closest("[data-quiz]")));
await page.keyboard.press("Escape");
const back = await page.evaluate(() => document.activeElement?.dataset.testid);
check("F6 goes to the quiz, Esc goes back to the terminal", inQuiz && back === "terminal-input", back);

/* ---- several questions ---- */
await load("quiz-count");
check("One quiz at a time, the oldest, with a count", (await page.getByTestId("quiz").count()) === 1 && (await page.getByTestId("quiz-more").innerText()).includes("2 more") && (await page.getByTestId("quiz").getAttribute("data-quiz")) === "q1", await page.getByTestId("quiz-more").innerText());
const option = page.getByTestId("quiz").getByRole("button", { name: "Per project" });
const point = await option.boundingBox();
const cardH = (await page.getByTestId("quiz").boundingBox()).height;
await page.mouse.click(point.x + 10, point.y + 10);
await page.getByTestId("pause").waitFor();
const start = Date.now();
await page.mouse.click(point.x + 10, point.y + 10); // the double click
await page.mouse.click(point.x + 10, point.y + 10);
const pauseH = (await page.getByTestId("pause").boundingBox()).height;
const stillTwo = await needs();
await page.getByTestId("quiz").waitFor();
const waited = Date.now() - start;
check("A second click during the pause lands on nothing", stillTwo === 2, `✋ ${stillTwo}`);
check("The pause lasts about a second, then the next quiz comes", waited > 700 && waited < 1600, `${waited}ms`);
check("The pause keeps the quiz's height, so nothing jumps", Math.abs(pauseH - cardH) <= 24, `quiz ${Math.round(cardH)}px, pause ${Math.round(pauseH)}px`);

/* ---- one answer clears every surface ---- */
await load("quiz-choices");
await page.getByTestId("needs-you-button").click();
await page.locator('[data-testid="inbox-card"]').getByRole("button", { name: "Per runner" }).click();
check(
  "One answer in the Inbox clears the quiz, ✋, the card, the row and the tab's count",
  (await needs()) === 0 &&
    (await page.getByTestId("quiz").count()) === 0 &&
    (await page.locator('[data-testid="inbox-card"]').count()) === 0 &&
    (await page.getByTestId("word-dev7").count()) === 0 &&
    (await page.getByTestId("tab-count-steward").count()) === 0,
  "all gone",
);

/* ---- an agent answers first ---- */
await load("quiz-choices");
await page.evaluate(() => window.tq.pretend(0));
await page.getByTestId("closing").waitFor();
check(
  "An agent answering closes the quiz quietly: one line, no ✋, no Update",
  (await needs()) === 0 &&
    (await page.getByTestId("quiz").count()) === 0 &&
    (await page.getByTestId("update").count()) === 0 &&
    (await page.locator('[role="alert"]').count()) === 0,
  await page.getByTestId("closing").locator("p").innerText(),
);
await load("quiz-agent-closed-typing");
check("What you typed survives an agent's answer", (await page.getByTestId("closing-typed").locator("textarea").inputValue()).length > 0, await page.getByTestId("closing-typed").locator("textarea").inputValue());

/* ---- a background turn ---- */
await load("strip-background");
const text = await page.locator(".tq-strip-says").innerText();
await page.evaluate(() => window.tq.pretend(0));
await page.waitForTimeout(50);
check("A background turn leaves the strip and ✋ alone", (await page.locator(".tq-strip-says").innerText()) === text && (await needs()) === 1, "strip kept");

/* ---- the setting ---- */
await load("each-change");
const was = await needs();
await page.evaluate(() => window.tq.pretend(0));
await page.waitForTimeout(50);
check("Setting from each session moves open questions at once, and no count changes", (await page.getByTestId("quiz").count()) === 0 && (await needs()) === was, `✋ ${was}`);
await page.getByRole("treeitem", { name: /developer 7/ }).click();
check("From each session, the quiz waits in the task's tab", (await page.getByTestId("quiz").count()) === 1, "drawn");
await load("top-answer-in-task");
check("From the top, the task's own tab can answer too", (await page.getByTestId("quiz").count()) === 1, "drawn");

/* ---- elsewhere ---- */
await load("quiz-elsewhere");
check("With the top tab showing something else, nothing switches and the count holds", (await page.getByTestId("quiz").count()) === 0 && (await page.locator(".tq-tab[aria-selected=true]").innerText()).includes("Settings") && (await hand()) === 3, "stays on Settings");

/* ---- a folded task ---- */
await load("row-folded");
check("A collapsed task still shows its state", (await page.locator('[data-row="arch2"]').innerText()).includes("asking"), (await page.locator('[data-row="arch2"]').innerText()).replace(/\s+/g, " "));

/* ---- a failed redirect ---- */
await load("inbox-failed");
check("A failed redirect falls back to the pane and the Inbox, with no top-tab quiz, and is said once", (await page.getByTestId("quiz").count()) === 0 && (await page.locator('[data-testid="inbox-card"]').count()) === 1 && (await page.getByTestId("update").count()) === 1, "one card, one Update");
await page.getByRole("treeitem", { name: /developer 7/ }).click();
check("A failed redirect can be answered in the task's pane", (await page.getByTestId("quiz").count()) === 1, "drawn");

/* ---- every quiz says why ---- */
let noWhy = [];
for (const s of ids) {
  await load(s);
  const quizzes = await page.locator("[data-quiz]").count();
  if (quizzes > 0 && (await page.locator("[data-quiz] .tq-why").count()) === 0) noWhy.push(s);
}
check("Every quiz says why it reached you", noWhy.length === 0, noWhy.join() || "all");

/* ---- no overflow ---- */
let spill = [];
for (const s of ids) {
  await load(s);
  const over = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  if (over) spill.push(s);
}
check("No state scrolls the window sideways", spill.length === 0, spill.join() || "none");
check("No script error in any state", errors.length === 0, errors[0] ?? "none");

await browser.close();
writeFileSync(resolve(here, "checks.json"), `${JSON.stringify(results, null, 2)}\n`);
console.log(`${results.filter((r) => r.pass).length}/${results.length} pass`);
