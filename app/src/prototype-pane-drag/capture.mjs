// PROTOTYPE, throwaway (#4583): the scenes mid-drag, as screenshots. From app/: node src/prototype-pane-drag/capture.mjs
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright";
const here = dirname(fileURLToPath(import.meta.url));
const url = pathToFileURL(resolve(here, "PROTOTYPE-artifact.html")).href;
const browser = await chromium.launch({ executablePath: process.env.PD_CHROMIUM || undefined });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, colorScheme: process.env.PD_LIGHT ? "light" : "dark" });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const open = async (q) => { await page.goto(`${url}?${q}`); await page.waitForTimeout(350); };
const shot = (name) => page.screenshot({ path: resolve(here, "screenshots", `${name}.png`) });
const pane = (name) => page.locator(".pd-pane", { has: page.locator(".ot-pane-name", { hasText: new RegExp(`^${name}$`) }) }).first();
const box = async (loc) => (await loc.boundingBox());
/** Press on `from`, drag to a point of `to`'s box (fx, fy in 0..1), hold, shoot, then drop or cancel. */
async function drag(from, to, fx, fy, name, end = "drop") {
  const a = await box(from);
  await page.mouse.move(a.x + Math.min(30, a.width / 2), a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(a.x + 40, a.y + a.height / 2 + 10, { steps: 3 });
  const b = await box(to);
  await page.mouse.move(b.x + b.width * fx, b.y + b.height * fy, { steps: 10 });
  await page.waitForTimeout(150);
  await page.mouse.move(b.x + b.width * fx + 1, b.y + b.height * fy, { steps: 1 });
  await page.waitForTimeout(150);
  await shot(name);
  if (end === "drop") await page.mouse.up();
  else { await page.keyboard.press("Escape"); await page.mouse.up(); }
  await page.waitForTimeout(200);
}
const row = (name) => page.locator(".pd-row", { has: page.locator(".ot-row-name", { hasText: new RegExp(`^${name}$`) }) }).first();
const grip = (name) => page.locator(".pd-grip", { hasText: name }).first();

for (const v of (process.env.PD_VS ?? "A,B,C").split(",")) {
  await open(`s=split&v=${v}`);
  await shot(`01-split-${v}`);
  if (v !== "B") {
    await drag(row("reviewer 1"), pane("steward 4"), 0.9, 0.5, `02-row-to-edge-${v}`);
    await shot(`03-row-dropped-${v}`);
    await open(`s=split&v=${v}`);
    await drag(grip("developer 7"), pane("steward 4"), 0.5, 0.5, `04-pane-to-centre-${v}`);
  } else {
    await drag(row("reviewer 1"), page.locator('.pd-pane:has(.ot-pane-name:text-is("steward 4")) [data-target-side="right"]'), 0.5, 0.5, `02-row-to-edge-${v}`);
    await shot(`03-row-dropped-${v}`);
    await open(`s=split&v=${v}`);
    await drag(grip("developer 7"), page.locator('.pd-pane:has(.ot-pane-name:text-is("steward 4")) [data-target-side="centre"]'), 0.5, 0.5, `04-pane-to-centre-${v}`);
  }
  await shot(`05-pane-swapped-${v}`);
  await open(`s=keys&v=${v}`);
  if (v === "C") { await grip("developer 7").focus(); await page.keyboard.press("Enter"); await page.waitForTimeout(200); }
  else { await page.waitForTimeout(200); await page.keyboard.press("ArrowRight"); await page.waitForTimeout(150); }
  await shot(`06-keys-${v}`);
}
// A only: tile, tabs, other workspace, limits.
await open("s=tile&v=A");
await shot("07-tile-before");
await page.locator(".pd-tile").click();
await page.waitForTimeout(250);
await shot("08-tile-after");
await open("s=tabs&v=A");
await drag(grip("developer 7"), page.locator(".pd-strip-end"), 0.5, 0.5, "09-pane-to-strip");
await shot("10-pane-made-a-tab");
await open("s=tabs&v=A");
await drag(page.locator('.pd-tab:has-text("reviewer 1") .ot-tab-press'), pane("steward 4"), 0.5, 0.92, "11-tab-into-split");
await shot("12-tab-merged");
await open("s=cross&v=A&x=allow");
await drag(row("steward 2"), pane("developer 7"), 0.5, 0.9, "13-other-ws-allow");
await shot("14-other-ws-dropped");
await open("s=cross&v=A&x=refuse");
await drag(row("steward 2"), pane("developer 7"), 0.5, 0.9, "15-other-ws-refuse", "cancel");
await open("s=limits&v=A");
await drag(row("reviewer 1"), pane("developer 9"), 0.92, 0.5, "16-too-narrow", "cancel");
await drag(row("reviewer 1"), pane("vim"), 0.5, 0.5, "17-job-no-swap", "cancel");
console.log("errors:", errors.length ? errors : "none");
await browser.close();
