// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#5227): draws the one notice's variants to `screenshots/`, light and dark,
 * and checks what a picture can't show: an ask arriving never takes focus, each box is masked, a
 * hostile command stays inert, and no value reaches the markup.
 * From app/: node src/prototype-secret-ask/capture.mjs [path to the built page]
 * In a sandboxed chat: SA_CHROMIUM=<headless shell> SA_ARGS="--single-process --no-zygote".
 */
import { mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const built = resolve(process.argv[2] ?? resolve(here, "PROTOTYPE-artifact.html"));
const ids = [...readFileSync(resolve(here, "model.ts"), "utf8").matchAll(/^    id: "([a-z0-9-]+)",$/gm)].map((m) => m[1]);
const url = (s, more = "") => `${pathToFileURL(built).href}?s=${s}&bare${more}`;
const shots = resolve(here, "screenshots");
rmSync(shots, { recursive: true, force: true });
mkdirSync(shots, { recursive: true });

const browser = await chromium.launch({
  executablePath: process.env.SA_CHROMIUM || undefined,
  args: (process.env.SA_ARGS ?? "").split(" ").filter(Boolean),
});
/* Tall, so the whole notice shows; `window-` shots are the ordinary window. */
const context = await browser.newContext({ viewport: { width: 1280, height: 1180 }, colorScheme: "dark" });
const page = await context.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
const load = async (s, more = "") => {
  await page.goto(url(s, more));
  await page.waitForSelector(".tq-root");
};
const check = (ok, said) => console.log(`${ok ? "pass" : "FAIL"}  ${said}`);

for (const appearance of ["dark", "light"]) {
  await page.emulateMedia({ colorScheme: appearance });
  let n = 0;
  for (const s of ids) {
    n += 1;
    const at = String(n).padStart(2, "0");
    await load(s, "&at=quiz");
    await page.screenshot({ path: resolve(shots, `${at}-${s}-${appearance}.png`) });
  }
  /* The first variant on every surface: the strip folded until Fill, then opened; the Inbox. */
  await load("one-notice", "&at=strip");
  await page.screenshot({ path: resolve(shots, `one-notice-strip-folded-${appearance}.png`) });
  await load("one-notice", "&at=strip&open");
  await page.screenshot({ path: resolve(shots, `one-notice-strip-open-${appearance}.png`) });
  await load("one-notice", "&at=inbox");
  await page.screenshot({ path: resolve(shots, `one-notice-inbox-${appearance}.png`) });
  await page.setViewportSize({ width: 1280, height: 820 });
  await load("one-notice", "&at=quiz");
  await page.screenshot({ path: resolve(shots, `one-notice-window-${appearance}.png`) });
  await page.setViewportSize({ width: 1280, height: 1180 });
}
await page.emulateMedia({ colorScheme: "dark" });

/* The picker: grouped by vault, with a filter on top. */
await load("pick-entry-empty", "&at=quiz");
await page.getByRole("button", { name: /Entry STRIPE_API_KEY/ }).or(page.getByLabel(/Entry STRIPE_API_KEY/)).first().click();
await page.waitForTimeout(150);
await page.screenshot({ path: resolve(shots, "picker-open-dark.png") });
await page.keyboard.press("Escape");

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
  check(focused === "terminal-input" && (await input.inputValue()) === "pnpm test", `an ask arriving keeps focus in the terminal (${surface})`);
}

/* Strip folded until Fill. */
await load("one-notice", "&at=strip");
check((await page.getByTestId("sa-body").count()) === 0, "the strip stays folded until Fill");
await page.getByRole("button", { name: "Fill" }).click();
check((await page.getByTestId("sa-body").count()) === 1, "Fill opens the strip's body");

/* Masked, and the value is never in the markup. */
await load("one-notice", "&at=quiz");
const box = page.locator('input[type="password"]');
check((await box.count()) === 1, "one value field for one key");
await box.fill("sk_test_SEEN");
const primary = page.getByTestId("sa-primary");
check((await primary.textContent()).trim() === "Fill and allow once", "Allow once is the default");

/* Several keys: a field each, one Allow, the action waits for both. */
await load("several-keys", "&at=quiz");
check((await page.locator('input[type="password"]').count()) === 2, "two missing keys, two value fields");
check((await page.getByTestId("sa-allow").count()) === 1, "two missing keys, one Allow");
check(await primary.isDisabled(), "the action waits for every key");

/* The disabled action reads, in both appearances. */
const ratio = async () =>
  page.evaluate(() => {
    const lum = (c) => {
      const box = document.createElement("canvas").getContext("2d");
      box.fillStyle = c;
      box.fillRect(0, 0, 1, 1);
      const [r, g, b] = box.getImageData(0, 0, 1, 1).data;
      const f = (v) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
      return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
    };
    const el = document.querySelector('[data-testid="sa-primary"]');
    const text = lum(getComputedStyle(el).color);
    let under = el;
    while (under && ["rgba(0, 0, 0, 0)", "transparent"].includes(getComputedStyle(under).backgroundColor)) under = under.parentElement;
    const back = lum(getComputedStyle(under).backgroundColor);
    return (Math.max(text, back) + 0.05) / (Math.min(text, back) + 0.05);
  });
for (const appearance of ["dark", "light"]) {
  await page.emulateMedia({ colorScheme: appearance });
  await load("one-notice", "&at=quiz");
  const r = await ratio();
  check(r >= 4.5, `the disabled action's label meets AA in ${appearance}: ${r.toFixed(2)}:1`);
}
await page.emulateMedia({ colorScheme: "dark" });

/* No Allow where none may be given, and Fill still works. */
for (const s of ["no-program", "interpreter", "policy-lock", "tools-lock", "runner"]) {
  await load(s, "&at=quiz");
  await page.locator('input[type="password"]').fill("x");
  const label = (await primary.textContent()).trim();
  check((await page.getByTestId("sa-allow").count()) === 0 && (await primary.isEnabled()), `${s}: no Allow, ${label} works`);
}
await load("runner", "&at=quiz");
check((await primary.textContent()).trim() === "Store here", "a runner's action reads Store here");

/* The hostile command: every control drawn, nothing obeyed, and the facts are the app's alone. */
await load("hostile-command", "&at=quiz");
const command = page.getByTestId("sa-command");
const shown = await command.textContent();
check(![...shown].some((c) => c.codePointAt(0) < 0x20 || [0x7f, 0x202e, 0x200b].includes(c.codePointAt(0))) && shown.includes("ESC") && shown.includes("U+202E"), "the hostile command shows its control characters as symbols");
check((await page.getByTestId("sa-facts").textContent()).includes("/usr/bin/curl") && !(await page.getByTestId("sa-facts").textContent()).includes("exfil"), "the chat's words never reach the app's panel");
const inside = await command.evaluate((el) => el.querySelectorAll("a, button, img, script").length);
check(inside === 0, "the command holds no live element");

console.log(errors.length ? `errors:\n${errors.join("\n")}` : "no page errors");
await browser.close();
