import { $, $$, browser, expect } from "@wdio/globals";
import { READY } from "../harness.js";
import { endChat, endEveryChat, pressAndStart } from "../opening.js";

/**
 * **The token and time limits' lines, and the settings set's native controls, in the real
 * WebView** (#1545 line 2; #626's train 27 follow-up 2).
 *
 * What jsdom cannot say: how the note under the limits table wraps in the page at a wide and a
 * narrow window, how a task's row reads "stopped at its time limit" in its tab's menu, and what
 * the engine's own keys do to a native `<select>` and range. `DispatchLimits.test.tsx`,
 * `shownState.test.ts`, `TabChip.window.test.tsx` and `gallery.test.tsx` hold the rest.
 *
 * **The task is pretended**, as `tab-chip.e2e.ts` pretends its tasks (`src/e2eTasks.ts`): a
 * task purlis stopped at its time limit needs a persona chat, a core that dispatched it and a
 * limit that ran out, none of which this suite's fake harness makes. The row, its word and the
 * menu it is in are the app's own from there.
 *
 * **The session's token line is not here**: the footer says it only once the session's own
 * limit numbers reach the window, and the pretend seam carries tasks alone (listed on #1545).
 *
 * **It leaves the window as it found it**: one app process serves the whole run. Nothing here
 * writes a setting (an override picked is not written until a limit is typed), the text size it
 * steps is stepped back, and the chat it opens is ended.
 */

const STRIP = '[data-strip="Tabs"]';
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
const NOTE = '[data-testid="dispatch-tokens-time-note"]';

type Box = { left: number; right: number; top: number; bottom: number };

/** Opens Settings at the Project level through the palette, as the operator does. */
async function openProjectSettings(): Promise<void> {
  await browser.keys(["F2"]);
  await $(PALETTE).waitForDisplayed({ timeout: 20_000 });
  const box = await $("#palette-query");
  await browser.waitUntil(async () => box.isFocused(), {
    timeout: 10_000,
    timeoutMsg: "the palette did not take the keyboard when it opened",
  });
  await box.addValue("Project settings");
  await browser.waitUntil(
    async () =>
      browser.execute(
        () =>
          document
            .querySelector('[role="option"][aria-selected="true"] .palette-title')
            ?.textContent?.startsWith("Project settings") ?? false,
      ),
    { timeout: 10_000, timeoutMsg: "the palette did not offer Project settings…" },
  );
  await browser.keys(["Enter"]);
  await $('nav[aria-label="Groups"]').waitForDisplayed({ timeout: 20_000 });
}

/** Presses the level called `level` on the switcher, then the nav's group called `group`. */
async function goTo(level: string, group: string): Promise<void> {
  await browser.execute((called: string) => {
    const radio = [
      ...document.querySelectorAll<HTMLElement>(
        '[role="radiogroup"][aria-label="Level"] [role="radio"]',
      ),
    ].find((one) => one.textContent?.trim() === called);
    if (radio?.getAttribute("aria-checked") !== "true") radio?.click();
  }, level);
  await browser.waitUntil(
    async () =>
      browser.execute((called: string) => {
        const button = [
          ...document.querySelectorAll<HTMLElement>('nav[aria-label="Groups"] button'),
        ].find((one) => one.textContent?.trim() === called);
        button?.click();
        return button?.getAttribute("aria-current") === "true";
      }, group),
    { timeout: 20_000, interval: 250, timeoutMsg: `Settings never showed ${level} › ${group}` },
  );
}

async function windowIs(width: number, height: number): Promise<void> {
  await browser.setWindowSize(width, height);
  let last = -1;
  await browser.waitUntil(
    async () => {
      const now = await browser.execute(() => window.innerWidth);
      const settled = now === last;
      last = now;
      return settled;
    },
    { timeout: 10_000, interval: 200, timeoutMsg: "the window never settled on a width" },
  );
}

/** The note under the limits table, measured against the column it is drawn in. */
async function theNote() {
  return browser.execute((note: string) => {
    const box = (el: Element | null): Box | null => {
      if (!el) return null;
      const { left, right, top, bottom } = el.getBoundingClientRect();
      return { left, right, top, bottom };
    };
    const said = document.querySelector<HTMLElement>(note);
    return {
      text: said?.textContent ?? "",
      note: box(said),
      body: box(said?.closest(".ui-settings-body") ?? null),
      cut: said ? said.scrollWidth > said.clientWidth + 1 : true,
      pageScrolls: document.documentElement.scrollWidth > window.innerWidth + 1,
    };
  }, NOTE);
}

/** The tabs on the strip, by name. */
async function tabNames(): Promise<string[]> {
  return browser.execute(
    (strip: string) =>
      [...(document.querySelector(strip)?.querySelectorAll('[role="tab"]') ?? [])].map(
        (tab) => tab.querySelector(".tab-name")?.textContent ?? "",
      ),
    STRIP,
  );
}

describe("the limits' lines and the set's native controls, in the real WebView", () => {
  let was = { width: 1280, height: 800 };
  let wereAlreadyOpen: string[] = [];

  before(async () => {
    await endEveryChat();
    wereAlreadyOpen = await tabNames();
    was = await browser.getWindowSize();
    await windowIs(1280, 800);
  });

  after(async () => {
    const tried = async (step: () => Promise<unknown>) => {
      try {
        await step();
      } catch {
        // The next step still runs.
      }
    };
    await tried(() =>
      browser.execute(() =>
        window.dispatchEvent(new CustomEvent("purlis-e2e-tasks", { detail: [] })),
      ),
    );
    await tried(() => browser.keys("Escape"));
    await tried(() => browser.setWindowSize(was.width, was.height));
    for (let round = 0; round < 5; round++) {
      const left = (await tabNames()).filter((tab) => !wereAlreadyOpen.includes(tab));
      if (left.length === 0) break;
      for (const name of left) await tried(() => endChat(`End chat ${name}`));
      await browser.pause(500);
    }
  });

  it("draws the note under the limits table whole, inside its column, wide and narrow", async () => {
    await openProjectSettings();
    await goTo("Project", "Dispatch");
    await $(NOTE).waitForDisplayed({ timeout: 20_000 });

    for (const [width, height] of [
      [1280, 800],
      [900, 700],
    ] as const) {
      await windowIs(width, height);
      const seen = await theNote();
      expect(seen.text).toContain("Tokens per session is not enforced yet");
      expect(seen.text).toContain("a harness that reports no tokens is not counted");
      const { note, body } = seen;
      if (note === null || body === null)
        throw new Error(`the note or its column is not drawn at ${width} px`);
      // Wrapped inside the column, never past it, and the page does not scroll sideways.
      expect(note.left).toBeGreaterThanOrEqual(body.left - 1);
      expect(note.right).toBeLessThanOrEqual(body.right + 1);
      expect(seen.cut).toBe(false);
      expect(seen.pageScrolls).toBe(false);
    }
    await windowIs(1280, 800);
  });

  it("moves the native select of Add an override with the engine's own arrow key (#626)", async () => {
    await goTo("Project", "Dispatch");
    await $(NOTE).waitForDisplayed({ timeout: 20_000 });
    const before = await browser.execute(() => {
      const select = [...document.querySelectorAll<HTMLSelectElement>("select")].find(
        (one) => one.labels?.[0]?.textContent?.trim() === "Override for",
      );
      select?.focus();
      return {
        value: select?.value ?? "",
        options: select ? [...select.options].map((one) => one.value) : [],
        focused: select !== undefined && document.activeElement === select,
      };
    });
    // The fixture project has two workspaces and two personas to offer.
    expect(before.options.length).toBeGreaterThanOrEqual(2);
    expect(before.focused).toBe(true);
    await browser.keys(["ArrowDown"]);
    await browser.waitUntil(
      async () =>
        browser.execute(
          (from: string) => (document.activeElement as HTMLSelectElement | null)?.value !== from,
          before.value,
        ),
      { timeout: 5_000, timeoutMsg: "the arrow key did not move the select's pick" },
    );
    // A pick writes nothing: Add an override only drafts a row, and nothing is written until a
    // limit is typed into it.
  });

  it("steps the native range of the text size with the arrow keys, and back (#626)", async () => {
    await goTo("You", "Text");
    const read = () =>
      browser.execute(() => {
        const range = document.querySelector<HTMLInputElement>('input[type="range"]');
        return {
          value: range?.valueAsNumber ?? Number.NaN,
          most: Number(range?.max ?? Number.NaN),
          said: range?.getAttribute("aria-valuetext") ?? "",
          focused: range !== null && document.activeElement === range,
        };
      });
    await browser.execute(() =>
      document.querySelector<HTMLInputElement>('input[type="range"]')?.focus(),
    );
    const before = await read();
    expect(before.focused).toBe(true);
    // At its top a step right does nothing, so step the way there is room.
    const [there, back] =
      before.value < before.most ? ["ArrowRight", "ArrowLeft"] : ["ArrowLeft", "ArrowRight"];
    await browser.keys([there]);
    await browser.waitUntil(async () => (await read()).value !== before.value, {
      timeout: 5_000,
      timeoutMsg: `${there} did not move the text size`,
    });
    const moved = await read();
    expect(Math.abs(moved.value - before.value)).toBe(1);
    expect(moved.said).toBe(`${moved.value} pixels`);
    await browser.keys([back]);
    await browser.waitUntil(async () => (await read()).value === before.value, {
      timeout: 5_000,
      timeoutMsg: `${back} did not put the text size back`,
    });
  });

  it("says a task purlis stopped at its time limit in its tab's menu, whole", async () => {
    const known = new Set(await tabNames());
    await pressAndStart("New tab");
    await browser.waitUntil(async () => (await tabNames()).some((name) => !known.has(name)), {
      timeout: 30_000,
      timeoutMsg: "the chat this spec opened never came onto the strip",
    });
    await browser.waitUntil(
      async () => {
        const panes = await $$('[data-testid="pane"]').getElements();
        return panes.length > 0 && (await panes[0].$(".xterm-rows").getText()).includes(READY);
      },
      { timeout: 30_000, interval: 250, timeoutMsg: `the pane never showed ${READY}` },
    );
    const asker = await browser.execute(() =>
      Number(document.querySelector('[data-testid="pane"]')?.getAttribute("data-session") ?? -1),
    );
    expect(asker).toBeGreaterThan(0);

    await browser.execute((of: number) => {
      window.dispatchEvent(
        new CustomEvent("purlis-e2e-tasks", {
          detail: [
            {
              workspace: "another-workspace",
              chat: {
                session: 9100,
                name: "9100",
                cwd: null,
                harness: "claude",
                in_front: false,
                resumed: null,
                fresh: null,
                profile: null,
                persona: null,
                unreported: null,
                card: null,
                guessed: null,
                pinned: false,
                label: "a task past its time",
                from: {
                  chat: of,
                  name: "its session",
                  workspace: "another-workspace",
                  task: true,
                  tab: false,
                  reported: true,
                  unreported: false,
                  outcome: "stopped_at_limit",
                },
              },
            },
          ],
        }),
      );
    }, asker);

    // The pointer rests on the chip, as a person's would (dispatched: see `tab-chip.e2e.ts`).
    await browser.waitUntil(
      async () =>
        browser.execute(
          (strip: string) => document.querySelector(`${strip} .tab-tasks-counts`) !== null,
          STRIP,
        ),
      { timeout: 10_000, timeoutMsg: "the tab never wore a chip for the pretended task" },
    );
    await browser.execute((strip: string) => {
      const counts = document.querySelector<HTMLElement>(`${strip} .tab-tasks-counts`);
      if (!counts) return;
      const at = counts.getBoundingClientRect();
      const send = (type: string, dx = 0) =>
        counts.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            cancelable: true,
            pointerId: 1,
            pointerType: "mouse",
            isPrimary: true,
            clientX: at.left + at.width / 2 + dx,
            clientY: at.top + at.height / 2,
          }),
        );
      send("pointerover");
      send("pointermove", 2);
    }, STRIP);
    await browser.waitUntil(
      async () =>
        browser.execute(() => document.querySelector('[role="menu"].tasks-menu') !== null),
      { timeout: 5_000, timeoutMsg: "resting on the chip opened no menu" },
    );
    // The finished task is in the fold: opened in place, as a click on it does.
    await browser.execute(() =>
      document.querySelector<HTMLElement>('[role="menu"].tasks-menu .tasks-menu-fold')?.click(),
    );
    let seen: { words: string[]; cut: string[]; inside: boolean } = {
      words: [],
      cut: [],
      inside: false,
    };
    await browser.waitUntil(
      async () => {
        seen = await browser.execute(() => {
          const menu = document.querySelector<HTMLElement>('[role="menu"].tasks-menu');
          const words = [...(menu?.querySelectorAll<HTMLElement>(".shown-state .word") ?? [])];
          const at = menu?.getBoundingClientRect();
          return {
            words: words.map((word) => word.textContent ?? ""),
            cut: words
              .filter((word) => word.scrollWidth > word.clientWidth + 1)
              .map((word) => word.textContent ?? ""),
            inside:
              at !== undefined &&
              at.left >= 0 &&
              at.right <= window.innerWidth &&
              at.bottom <= window.innerHeight,
          };
        });
        return seen.words.includes("stopped at its time limit");
      },
      {
        timeout: 5_000,
        timeoutMsg: `the menu never said "stopped at its time limit": ${JSON.stringify(seen)}`,
      },
    );
    expect(seen.cut).toEqual([]);
    expect(seen.inside).toBe(true);
  });
});
