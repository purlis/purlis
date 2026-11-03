// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#5070): draws every state on every surface to `screenshots/`, and checks
 * the one rule a picture can't show: an ask arriving never takes focus.
 * From app/: node src/prototype-secret-ask/capture.mjs [path to the built page]
 * In a sandboxed chat: SA_CHROMIUM=<headless shell> SA_ARGS="--single-process --no-zygote".
 */
import { mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const built = resolve(process.argv[2] ?? resolve(here, "PROTOTYPE-artifact.html"));
const ids = [...readFileSync(resolve(here, "model.ts"), "utf8").matchAll(/^    id: "([a-z0-9-]+)",$/gm)].map((m) => m[1]);
const url = (s, more = "") => `${pathToFileURL(built).href}?s=${s}&bare${more}`;
const shots = resolve(here, "screenshots");
mkdirSync(shots, { recursive: true });

const browser = await chromium.launch({
  executablePath: process.env.SA_CHROMIUM || undefined,
  args: (process.env.SA_ARGS ?? "").split(" ").filter(Boolean),
});
const context = await browser.newContext({ viewport: { width: 1280, height: 820 }, colorScheme: "dark" });
const page = await context.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
const load = async (s, more) => {
  await page.goto(url(s, more));
  await page.waitForSelector(".tq-root");
};

let n = 0;
for (const s of ids) {
  n += 1;
  const at = String(n).padStart(2, "0");
  for (const surface of ["quiz", "strip", "inbox"]) {
    await load(s, `&at=${surface}`);
    await page.screenshot({ path: resolve(shots, `${at}-${s}-${surface}.png`) });
  }
}
await load("declared", "&at=strip&closed");
await page.screenshot({ path: resolve(shots, "strip-folded.png") });
await page.goto(url("declared", "&at=quiz").replace("&bare", ""));
await page.waitForSelector(".tq-bar");
await page.screenshot({ path: resolve(shots, "bar.png") });
await page.emulateMedia({ colorScheme: "light" });
for (const [s, surface] of [["declared", "quiz"], ["undeclared", "inbox"], ["replace-linked", "strip"], ["pick-entry", "quiz"]]) {
  await load(s, `&at=${surface}`);
  await page.screenshot({ path: resolve(shots, `light-${s}-${surface}.png`) });
}
await page.emulateMedia({ colorScheme: "dark" });

/* An ask arriving never takes focus, on any surface. */
for (const surface of ["strip", "quiz"]) {
  await load("arrives-while-typing", `&at=${surface}`);
  const input = page.getByTestId("terminal-input");
  await input.click();
  await page.keyboard.press("End");
  await page.evaluate(() => window.sa.pretend());
  await page.locator('[data-testid="sa-body"], [data-testid="strip"]').first().waitFor();
  await page.keyboard.type("st");
  const focused = await page.evaluate(() => document.activeElement?.dataset.testid);
  const ok = focused === "terminal-input" && (await input.inputValue()) === "pnpm test";
  console.log(`${ok ? "pass" : "FAIL"}  an ask arriving keeps focus in the terminal (${surface}): "${await input.inputValue()}"`);
  await page.screenshot({ path: resolve(shots, `focus-${surface}-after.png`) });
}

/* The masked box never shows its words. */
await load("new-value", "&at=quiz");
const box = page.getByTestId("quiz").locator('input[type="password"]');
await box.fill("sk_test_PROTOTYPE");
const typeOk = (await box.getAttribute("type")) === "password";
console.log(`${typeOk ? "pass" : "FAIL"}  the value box is masked`);
await page.screenshot({ path: resolve(shots, "masked-typed.png") });

console.log(errors.length ? `errors:\n${errors.join("\n")}` : "no page errors");
await browser.close();
