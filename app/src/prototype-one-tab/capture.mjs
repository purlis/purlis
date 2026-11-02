// PROTOTYPE, throwaway (#4582): every scene × strip treatment, as screenshots.
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright";
const here = dirname(fileURLToPath(import.meta.url));
const page_ = pathToFileURL(resolve(here, "PROTOTYPE-artifact.html")).href;
const browser = await chromium.launch({ executablePath: process.env.OT_CHROMIUM || undefined, args: (process.env.OT_ARGS ?? "").split(" ").filter(Boolean) });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, colorScheme: process.env.OT_LIGHT ? "light" : "dark" });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const scenes = (process.env.OT_SCENES ?? "one,split,tabs,empty,fresh,plane,handoff,ask").split(",");
const vs = (process.env.OT_VS ?? "A,B,C").split(",");
let n = 0;
for (const s of scenes) for (const v of vs) {
  await page.goto(`${page_}?s=${s}&v=${v}`);
  await page.waitForTimeout(400);
  await page.screenshot({ path: resolve(here, "screenshots", `${String(++n).padStart(2, "0")}-${s}-${v}${process.env.OT_LIGHT ? "-light" : ""}.png`) });
}
// The sidebar pick, timed: 20 presses on rows not on screen.
await page.goto(`${page_}?s=one&v=A`);
await page.waitForTimeout(400);
const rows = page.locator('[role="treeitem"]');
const count = await rows.count();
for (let i = 0; i < 20; i++) { await rows.nth((i * 3 + 1) % count).click(); await page.waitForTimeout(60); }
console.log("pick:", await page.locator(".ot-bar-time").innerText());
console.log("errors:", errors.length ? errors : "none");
await browser.close();
