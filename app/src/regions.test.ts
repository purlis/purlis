// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  CATALOGUE,
  DEFAULT_ARRANGEMENT,
  forgetThisLaunch,
  inSlots,
  leastOf,
  LEGACY_KEY,
  MOST_PROJECTS,
  openView,
  PANEL,
  panelShown,
  placesOf,
  PROJECTS_MOST_BYTES,
  regionOf,
  REGION_IDS,
  remembered,
  settleLayout,
  shownIn,
  SIDES,
  SLOT_IDS,
  SLOTS,
  slotSize,
  HALVES,
  halvesSplit,
  useArrangement,
  rowWidth,
  slotLength,
  tabsOf,
  type Arrangement,
} from "./regions";
import { previewsDrawn, type ViewId } from "./sideViews";
import { setYourEditor } from "./yourEditor";
import {
  aboutThisMachine,
  GLOBAL,
  sayAboutThisMachine,
  usingTheDefaultLayout,
  type Reading,
} from "./windowprefs";

const PATH = "/home/op/.config/charter/layout.json";

/** Hands the window a layout file, as the initialization script does before anything runs. */
const handed = (layout: Partial<Reading>) => {
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: PATH, found: false, document: null, trouble: null, ...layout },
    theme: { path: "", found: false, document: null, trouble: null },
  };
};
/** A layout file holding `document`. */
const put = (document: unknown) => handed({ found: true, document });
/** The Panel's two halves as a file written before #1737 is drawn with them: open, at the
 *  default height (P-14). */
const THE_PANEL = [
  { id: "panel-left", side: "left", order: 0, collapsed: false },
  { id: "panel-right", side: "right", order: 0, collapsed: false },
];
const placed = (arrangement: ReturnType<typeof remembered>, id: string) =>
  arrangement.find((one) => one.id === id);
/** The navigation region's placement, which every arrangement has. */
const navigationOf = (arrangement: ReturnType<typeof remembered>) => {
  const found = placed(arrangement, "navigation");
  if (found === undefined) throw new Error("no navigation region");
  return found;
};

/** Every command the window sent the core, in order, and what the core answers. */
let sent: { cmd: string; args: unknown }[] = [];
let answer: (cmd: string) => unknown = () => null;

beforeEach(() => {
  globalThis.localStorage.clear();
  Reflect.deleteProperty(globalThis, GLOBAL);
  forgetThisLaunch();
  sayAboutThisMachine("layout", undefined);
  sent = [];
  answer = (cmd) => (cmd === "adopt_layout" ? true : null);
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    return answer(cmd);
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("the arrangement a window has never been told about", () => {
  it("is the two sides, navigation left and attention right, and the Panel's two halves (#1737)", () => {
    expect(remembered()).toEqual([
      { id: "navigation", side: "left", order: 0, collapsed: false },
      { id: "aside", side: "right", order: 0, collapsed: false },
      ...THE_PANEL,
    ]);
    expect(SIDES).toEqual(["left", "right"]);
  });

  it("names every region in the catalogue, so no region can exist with nowhere to be", () => {
    expect(DEFAULT_ARRANGEMENT.map((one) => one.id).sort()).toEqual([...REGION_IDS].sort());
  });

  it("puts every slot inside the bounds the slot itself allows", () => {
    // A default size outside its own slot's bounds is a window that lays out somewhere other
    // than where the arrangement says on its very first frame.
    for (const side of SLOT_IDS) {
      const size = slotSize(side, inSlots(DEFAULT_ARRANGEMENT)[side]);
      expect(size).toBeGreaterThanOrEqual(SLOTS[side].least);
      expect(size).toBeLessThanOrEqual(SLOTS[side].most);
    }
  });
});

describe("putting a region away", () => {
  it("puts it away and brings it back", () => {
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("navigation"));
    expect(placed(result.current.arrangement, "navigation")?.collapsed).toBe(true);

    act(() => result.current.toggle("navigation"));
    expect(placed(result.current.arrangement, "navigation")?.collapsed).toBe(false);
  });

  it("leaves every other region where it was", () => {
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("navigation"));

    expect(result.current.arrangement.filter((one) => one.collapsed).map((one) => one.id)).toEqual([
      "navigation",
    ]);
  });

  it("remembers the answer, so a project switch does not bring a region back", () => {
    // The regions are the WINDOW's layout, and a `PlaneView` that is not in front draws
    // nothing at all — so without this, looking at another project and back would undo it.
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("aside"));

    expect(remembered()).toEqual(result.current.arrangement);
    expect(placed(remembered(), "aside")?.collapsed).toBe(true);
  });
});

describe("moving a region", () => {
  it("puts it on the side it was moved to, and remembers", () => {
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.move("aside", "left", 1));

    expect(placed(result.current.arrangement, "aside")).toMatchObject({
      side: "left",
      order: 1,
    });
    expect(placed(remembered(), "aside")).toMatchObject({ side: "left", order: 1 });
  });

  it("leaves the side it came from empty rather than holding a place for it", () => {
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.move("aside", "left", 1));

    expect(inSlots(result.current.arrangement).right).toEqual([]);
  });
});

describe("how big each slot was left", () => {
  it("is remembered, which is what charter-app#141 deferred", () => {
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.resized({ left: 31 }));

    expect(placed(remembered(), "navigation")?.size).toBe(31);
    expect(slotSize("left", inSlots(remembered()).left)).toBe(31);
  });

  it("is the slot's, so every region drawn in it takes the width it was drawn at", () => {
    const { result } = renderHook(() => useArrangement());
    act(() => result.current.move("aside", "left", 1));

    act(() => result.current.resized({ left: 33 }));

    expect(placed(result.current.arrangement, "navigation")?.size).toBe(33);
    expect(placed(result.current.arrangement, "aside")?.size).toBe(33);
  });

  it("is not written to a region that was not drawn", () => {
    // A region that is away was not measured. Giving it the slot's width would be charter
    // deciding how wide it should come back, from a drag it had no part in.
    const { result } = renderHook(() => useArrangement());
    act(() => result.current.move("aside", "left", 1));
    act(() => result.current.toggle("aside"));

    act(() => result.current.resized({ left: 33 }));

    expect(placed(result.current.arrangement, "aside")?.size).toBeUndefined();
  });

  it("ignores a zero, because a collapsed slot measures zero and that is not a width", () => {
    const { result } = renderHook(() => useArrangement());
    act(() => result.current.resized({ left: 28 }));

    act(() => result.current.resized({ left: 0 }));

    expect(placed(result.current.arrangement, "navigation")?.size).toBe(28);
  });

  it("falls back on the catalogue until something has been dragged", () => {
    expect(slotSize("right", inSlots(DEFAULT_ARRANGEMENT).right)).toBe(CATALOGUE.aside.size);
  });

  it("is the first region DRAWN in the slot, not the first one placed there", () => {
    // A slot holding a region that is put away and one that is not takes the width of the one
    // the operator can see. Asking the first placed region would size the slot from something
    // that is not on screen, and the width would jump when it came back.
    const slots = inSlots([
      { id: "navigation", side: "left", order: 0, collapsed: true, size: 40 },
      { id: "aside", side: "left", order: 1, collapsed: false, size: 12 },
    ]);

    expect(slotSize("left", slots.left)).toBe(12);
  });
});

describe("a stored arrangement that is not what this build writes", () => {
  it("draws a region the document does not mention", () => {
    // A document written by an older build, or by hand. Losing a region with no way to notice
    // it went is worse than ignoring half a preference.
    put({ regions: [{ id: "navigation", side: "left", order: 0, collapsed: true }] });

    expect(placed(remembered(), "navigation")?.collapsed).toBe(true);
    expect(placed(remembered(), "aside")).toEqual({
      id: "aside",
      side: "right",
      order: 0,
      collapsed: false,
    });
  });

  it("drops a region this build does not have, and keeps the rest", () => {
    put({
      regions: [
        { id: "minimap", side: "left", order: 0, collapsed: false },
        { id: "aside", side: "left", order: 2, collapsed: false },
      ],
    });

    expect(remembered().map((one) => one.id)).toEqual([
      "navigation",
      "aside",
      "panel-left",
      "panel-right",
    ]);
    expect(placed(remembered(), "aside")).toMatchObject({ side: "left", order: 2 });
  });

  it("draws a region whose `collapsed` is not an answer", () => {
    // Only `true` puts a region away. `"yes"` is a document charter cannot read, and the
    // failure of a preference must not be the operator losing a region with no way to notice.
    put({ regions: [{ id: "aside", side: "right", order: 0, collapsed: "yes" }] });

    expect(placed(remembered(), "aside")?.collapsed).toBe(false);
  });

  it("uses the default side for a side that is not a side", () => {
    put({ regions: [{ id: "aside", side: "diagonal", order: 0, collapsed: false }] });

    expect(placed(remembered(), "aside")?.side).toBe("right");
  });

  it("uses the default order for an order that is not a number", () => {
    put({ regions: [{ id: "aside", side: "right", order: "first", collapsed: false }] });

    expect(placed(remembered(), "aside")?.order).toBe(0);
  });

  it("ignores a size that is not a percentage a panel could be given", () => {
    for (const size of [-5, 0, 140, "20%", null, Number.NaN]) {
      globalThis.localStorage.clear();
      put({ regions: [{ id: "navigation", side: "left", order: 0, collapsed: false, size }] });

      expect(placed(remembered(), "navigation")?.size).toBeUndefined();
    }
  });

  it("keeps a size that is one", () => {
    put({ regions: [{ id: "navigation", side: "left", order: 0, collapsed: false, size: 12.5 }] });

    expect(placed(remembered(), "navigation")?.size).toBe(12.5);
  });

  it("draws the default arrangement when the document is not one", () => {
    for (const held of [null, [], "a layout", 7, { regions: "none" }]) {
      put(held);

      expect(remembered()).toEqual(DEFAULT_ARRANGEMENT);
    }
  });

  it("skips an entry that is not an object at all", () => {
    put({ regions: [null, "navigation", ["aside"], { id: "aside", side: "left", order: 3 }] });

    expect(remembered().map((one) => one.id)).toEqual([
      "navigation",
      "aside",
      "panel-left",
      "panel-right",
    ]);
    expect(placed(remembered(), "aside")).toMatchObject({ side: "left", order: 3 });
  });
});

describe("a file that places the bottom region, which #1676 took out of the window", () => {
  it("is drawn without it, every other region where the file put it", () => {
    put({
      version: 2,
      regions: [
        { id: "navigation", side: "right", order: 0, collapsed: false, size: 30 },
        { id: "aside", side: "left", order: 0, collapsed: false },
        { id: "bottom", side: "bottom", order: 0, collapsed: true },
      ],
    });

    expect(remembered()).toEqual([
      { id: "navigation", side: "right", order: 0, collapsed: false, size: 30 },
      { id: "aside", side: "left", order: 0, collapsed: false },
      ...THE_PANEL,
    ]);
  });

  it("puts a region a file moved to the bottom on its own side, without a word", async () => {
    // Version 1 let a person move any region along the bottom. That slot is gone, so the
    // region goes back where it starts; nothing of theirs is lost and nothing is to fix. Its
    // size was a height there, so it is not taken as a width.
    const layout: Reading = {
      path: PATH,
      found: true,
      trouble: null,
      document: {
        version: 1,
        regions: [
          { id: "explorer", side: "left", order: 0, collapsed: false },
          { id: "aside", side: "bottom", order: 0, collapsed: false, size: 30 },
        ],
      },
    };
    handed(layout);

    expect(placed(remembered(), "aside")).toEqual({
      id: "aside",
      side: "right",
      order: 0,
      collapsed: false,
    });
    await settleLayout(layout);
    expect(aboutThisMachine()).toEqual([]);
  });

  it("is not a trouble to say: its content is the Changes view now", async () => {
    const layout: Reading = {
      path: PATH,
      found: true,
      trouble: null,
      document: {
        version: 1,
        regions: [
          { id: "explorer", side: "left", order: 0, collapsed: false },
          { id: "bottom", side: "bottom", order: 0, collapsed: false },
        ],
      },
    };

    await settleLayout(layout);

    expect(aboutThisMachine()).toEqual([]);
  });
});

describe("the layout file", () => {
  it("is what the first frame is drawn from, with nothing fetched", () => {
    put({
      version: 2,
      regions: [{ id: "navigation", side: "right", order: 1, collapsed: false, size: 22 }],
    });

    expect(placed(remembered(), "navigation")).toEqual({
      id: "navigation",
      side: "right",
      order: 1,
      collapsed: false,
      size: 22,
    });
    expect(sent).toEqual([]);
  });

  it("is where every change goes, in the order the window made them", async () => {
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("navigation"));
    act(() => result.current.move("aside", "left", 1));

    await vi.waitFor(() => expect(sent).toHaveLength(2));
    expect(sent.map((one) => one.cmd)).toEqual(["write_layout", "write_layout"]);
    const last = JSON.parse((sent[1].args as { text: string }).text);
    expect(last).toEqual({
      version: 2,
      regions: [
        { id: "navigation", side: "left", order: 0, collapsed: true },
        { id: "aside", side: "left", order: 1, collapsed: false },
        ...THE_PANEL,
      ],
      // The two text sizes live in the same file (charter-app#283), and a change to the
      // arrangement writes them as they stand.
      text: { window: 14, terminal: 13 },
    });
  });

  it("keeps the text sizes the file held when the arrangement changes", async () => {
    put({ version: 2, regions: [], text: { window: 18, terminal: 11 } });
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("aside"));

    await vi.waitFor(() => expect(sent).toHaveLength(1));
    const kept = JSON.parse((sent[0].args as { text: string }).text);
    expect(kept.text).toEqual({ window: 18, terminal: 11 });
  });

  it("keeps your editor (RC-20) when the arrangement changes", async () => {
    put({ version: 2, regions: [], editor: "zed" });
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("aside"));

    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(JSON.parse((sent[0].args as { text: string }).text).editor).toBe("zed");
  });

  it("is rewritten with your editor when it is chosen", async () => {
    act(() => setYourEditor("idea"));

    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].cmd).toBe("write_layout");
    expect(JSON.parse((sent[0].args as { text: string }).text).editor).toBe("idea");
  });

  it("that could not be read is drawn as the default, and the Inbox says why and where", async () => {
    handed({ found: true, trouble: `${PATH} is not JSON: expected value at line 1` });

    expect(remembered()).toEqual(DEFAULT_ARRANGEMENT);
    await settleLayout();

    const [said] = aboutThisMachine();
    expect(said.subject).toBe("layout");
    expect(said.detail).toContain("is not JSON");
    expect(said.detail).toContain("default arrangement");
    expect(said.remedy).toContain(PATH);
  });

  it("naming a region this build does not have is drawn without it, and says so", async () => {
    put({ version: 2, regions: [{ id: "minimap", side: "left", order: 0 }] });

    expect(remembered()).toEqual(DEFAULT_ARRANGEMENT);
    await settleLayout();

    expect(aboutThisMachine()).toHaveLength(1);
    expect(aboutThisMachine()[0].detail).toContain('"minimap" is not a region this purlis has');
  });

  it("says nothing when there is nothing wrong with it", async () => {
    put({ version: 2, regions: DEFAULT_ARRANGEMENT });

    await settleLayout();

    expect(aboutThisMachine()).toEqual([]);
  });

  it("that the core would not write is said, and the window keeps what the operator did", async () => {
    answer = (cmd) => {
      if (cmd === "write_layout") throw "purlis will not overwrite a layout it could not read";
      return null;
    };
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("aside"));

    expect(placed(result.current.arrangement, "aside")?.collapsed).toBe(true);
    await vi.waitFor(() => expect(aboutThisMachine()).toHaveLength(1));
    expect(aboutThisMachine()[0].detail).toContain("will not overwrite");
  });

  it("takes back what the Inbox said about it once a change has been kept", async () => {
    handed({ found: true, trouble: `${PATH} is not JSON` });
    await settleLayout();
    expect(aboutThisMachine()).toHaveLength(1);
    const { result } = renderHook(() => useArrangement());

    act(() => result.current.toggle("aside"));

    await vi.waitFor(() => expect(aboutThisMachine()).toEqual([]));
  });
});

describe("the arrangement web storage held before the file", () => {
  const legacy = [{ id: "navigation", side: "left", order: 0, collapsed: true }];

  it("is drawn on the first launch that finds no file, and moved into it", async () => {
    globalThis.localStorage.setItem(LEGACY_KEY, JSON.stringify({ regions: legacy }));
    handed({ found: false });

    expect(placed(remembered(), "navigation")?.collapsed).toBe(true);
    await settleLayout();

    expect(sent.map((one) => one.cmd)).toEqual(["adopt_layout"]);
    expect(JSON.parse((sent[0].args as { text: string }).text)).toMatchObject({
      version: 2,
      regions: [
        { id: "navigation", collapsed: true },
        { id: "aside" },
        { id: "panel-left" },
        { id: "panel-right" },
      ],
    });
    // Moved, so never read again.
    expect(globalThis.localStorage.getItem(LEGACY_KEY)).toBeNull();
    // And a project opened after the move still gets it, with the key gone.
    expect(placed(remembered(), "navigation")?.collapsed).toBe(true);
  });

  it("is kept where it is when the move fails, so the next launch tries again", async () => {
    globalThis.localStorage.setItem(LEGACY_KEY, JSON.stringify({ regions: legacy }));
    answer = () => {
      throw "the disk is full";
    };

    await settleLayout();

    expect(globalThis.localStorage.getItem(LEGACY_KEY)).not.toBeNull();
    expect(aboutThisMachine()[0].detail).toContain("the disk is full");
  });

  it("is not read at all once there is a file", async () => {
    globalThis.localStorage.setItem(LEGACY_KEY, JSON.stringify({ regions: legacy }));
    const read = vi.spyOn(globalThis.Storage.prototype, "getItem");
    put({ version: 2, regions: [] });

    expect(placed(remembered(), "navigation")?.collapsed).toBe(false);
    await settleLayout();

    expect(read).not.toHaveBeenCalled();
    expect(sent).toEqual([]);
  });

  it("draws the default when the webview will not read it", () => {
    vi.spyOn(globalThis.Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("storage is blocked");
    });

    expect(remembered()).toEqual(DEFAULT_ARRANGEMENT);
  });
});

describe("the arrangement as the slots it draws", () => {
  it("gives every slot a list, including one nothing is placed in", () => {
    expect(Object.keys(inSlots(DEFAULT_ARRANGEMENT)).sort()).toEqual([...SLOT_IDS].sort());
    expect(inSlots([{ id: "navigation", side: "left", order: 0, collapsed: false }]).right).toEqual(
      [],
    );
  });

  it("sorts a side by order", () => {
    const slots = inSlots([
      { id: "navigation", side: "left", order: 5, collapsed: false },
      { id: "aside", side: "left", order: 3, collapsed: false },
    ]);

    expect(slots.left.map((one) => one.id)).toEqual(["aside", "navigation"]);
  });

  it("breaks a tie the same way every launch", () => {
    // Two regions given the same order is a document a person wrote. Leaving the answer to
    // whatever order the array happened to be in would move the window between launches.
    const slots = inSlots([
      { id: "aside", side: "left", order: 0, collapsed: false },
      { id: "navigation", side: "left", order: 0, collapsed: false },
    ]);

    expect(slots.left.map((one) => one.id)).toEqual(["navigation", "aside"]);
  });

  it("counts what is drawn in a slot and not what is placed there", () => {
    const placedThere = [
      { id: "navigation" as const, side: "left" as const, order: 0, collapsed: true },
      { id: "aside" as const, side: "left" as const, order: 1, collapsed: false },
    ];

    expect(shownIn(placedThere).map((one) => one.id)).toEqual(["aside"]);
  });
});

/** What the window wrote last, as the file would hold it. */
const lastWritten = () => {
  const writes = sent.filter((one) => one.cmd === "write_layout");
  return JSON.parse((writes[writes.length - 1].args as { text: string }).text);
};

describe("a version 1 layout file, as every purlis before #1673 wrote it", () => {
  // `docs/design-system.md`'s own example, as it stood for version 1: a person who moved the
  // explorer right, swapped the attention region left and put the bottom bar away.
  const DOCUMENTED = {
    version: 1,
    regions: [
      { id: "explorer", side: "right", order: 0, collapsed: false, size: 22 },
      { id: "aside", side: "left", order: 0, collapsed: false },
      { id: "bottom", side: "bottom", order: 0, collapsed: true },
    ],
    text: { window: 15, terminal: 14 },
    editor: "zed",
    chats: { lines: 1, grouped: true, tabbed: true, away: false },
    dismissed: { "/home/me/project": ["pin-dormant:ide"] },
  };

  it("keeps every region where the person put it, the explorer's now the navigation region's", () => {
    put(DOCUMENTED);

    expect(remembered("/home/me/project")).toEqual([
      { id: "navigation", side: "right", order: 0, collapsed: false, size: 22 },
      { id: "aside", side: "left", order: 0, collapsed: false },
      ...THE_PANEL,
    ]);
  });

  it("is moved forward without a word in the Inbox", async () => {
    put(DOCUMENTED);

    await settleLayout();

    expect(aboutThisMachine()).toEqual([]);
  });

  it("is where every project starts, since version 1 had one arrangement for all of them", () => {
    put(DOCUMENTED);

    expect(remembered("/one")).toEqual(remembered("/two"));
    expect(placed(remembered("/two"), "navigation")).toMatchObject({ side: "right", size: 22 });
  });

  it("opens the left side on Chats, which sat above the explorer in version 1", () => {
    put(DOCUMENTED);

    expect(openView(navigationOf(remembered("/one")))).toBe("chats");
  });

  it("keeps a region the person had put away put away", () => {
    put({
      version: 1,
      regions: [
        { id: "explorer", side: "left", order: 0, collapsed: true, size: 31.5 },
        { id: "aside", side: "right", order: 0, collapsed: false, size: 24 },
        { id: "bottom", side: "bottom", order: 0, collapsed: false },
      ],
    });

    expect(placed(remembered("/one"), "navigation")).toEqual({
      id: "navigation",
      side: "left",
      order: 0,
      collapsed: true,
      size: 31.5,
    });
    expect(placed(remembered("/one"), "aside")?.size).toBe(24);
  });

  it("is written back as version 2 at the first change, keeping what else the file held", async () => {
    put(DOCUMENTED);
    const { result } = renderHook(() => useArrangement("/home/me/project"));

    act(() => result.current.toggle("aside"));

    await vi.waitFor(() => expect(sent).toHaveLength(1));
    const written = lastWritten();
    expect(written.version).toBe(2);
    // The bottom region is gone (#1676): what it drew is the Changes view.
    // The Panel's halves join it (#1737), open, as a file that never placed them is drawn.
    expect(written.regions.map((one: { id: string }) => one.id)).toEqual([
      "navigation",
      "aside",
      "panel-left",
      "panel-right",
    ]);
    expect(written.text).toEqual({ window: 15, terminal: 14 });
    expect(written.editor).toBe("zed");
    // The dismissals are the core's to keep (NO-2): a window's write never carries them.
    expect(written.dismissed).toBeUndefined();
  });

  it("is moved forward from web storage too, where the oldest builds kept it", () => {
    globalThis.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ regions: [{ id: "explorer", side: "right", order: 0, collapsed: true }] }),
    );

    expect(placed(remembered(), "navigation")).toMatchObject({ side: "right", collapsed: true });
  });
});

describe("each project's own arrangement", () => {
  it("is drawn from the file, and a project the file does not name starts from the machine's", () => {
    put({
      version: 2,
      regions: DEFAULT_ARRANGEMENT,
      projects: {
        "/one": {
          regions: [
            { id: "navigation", side: "left", order: 0, collapsed: true, view: "explorer" },
          ],
        },
      },
    });

    expect(placed(remembered("/one"), "navigation")).toMatchObject({
      collapsed: true,
      view: "explorer",
    });
    expect(remembered("/two")).toEqual(DEFAULT_ARRANGEMENT);
  });

  it("is written under the project, and only the projects this window arranged are sent", async () => {
    put({
      version: 2,
      regions: DEFAULT_ARRANGEMENT,
      projects: { "/other": { regions: DEFAULT_ARRANGEMENT } },
    });
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.toggle("navigation"));

    await vi.waitFor(() => expect(sent).toHaveLength(1));
    const written = lastWritten();
    // The core keeps `/other` from the file (`purlis_core::windowprefs::write_layout`), so a
    // second window arranging it meanwhile keeps what it did.
    expect(Object.keys(written.projects)).toEqual(["/one"]);
    expect(written.projects["/one"].regions[0]).toMatchObject({
      id: "navigation",
      collapsed: true,
    });
  });

  it("is the project's, so arranging one leaves an open project's alone", () => {
    const one = renderHook(() => useArrangement("/one"));
    const two = renderHook(() => useArrangement("/two"));

    act(() => one.result.current.toggle("navigation"));

    expect(placed(two.result.current.arrangement, "navigation")?.collapsed).toBe(false);
    expect(placed(remembered("/one"), "navigation")?.collapsed).toBe(true);
  });

  it("skips a project's entry that is not an arrangement, and says so", async () => {
    put({ version: 2, regions: DEFAULT_ARRANGEMENT, projects: { "/one": "left", "/two": null } });

    expect(remembered("/one")).toEqual(DEFAULT_ARRANGEMENT);
    await settleLayout();

    expect(aboutThisMachine()[0].detail).toContain('"/one"');
  });

  it("cannot reach a prototype through a project's name", () => {
    put(JSON.parse('{"version":2,"regions":[],"projects":{"__proto__":{"regions":[]}}}'));

    expect(remembered("__proto__")).toEqual(DEFAULT_ARRANGEMENT);
    expect(({} as Record<string, unknown>).regions).toBeUndefined();
  });
});

describe("the open view of a side (#1673)", () => {
  const navigation = navigationOf;

  it("is Chats until the person picks another", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    expect(openView(navigation(result.current.arrangement))).toBe("chats");
  });

  it("switches on a pick of another view, and the side stays open", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("explorer"));

    expect(navigation(result.current.arrangement)).toMatchObject({
      view: "explorer",
      collapsed: false,
    });
  });

  it("starts the right panel at 300px where the window has room, else at its 20% (R124)", () => {
    const right = inSlots(DEFAULT_ARRANGEMENT).right;
    expect(slotLength("right", right, 1600)).toBe("300px");
    expect(slotLength("right", right, 1280)).toBe("20%");
    const dragged = right.map((one) => ({ ...one, size: 25 }));
    expect(slotLength("right", dragged, 1600)).toBe("25%");
  });

  it("starts the bottom row at 34% in a tall window and at 30% in a short one (R126, D-2917-17)", () => {
    const panel = inSlots(DEFAULT_ARRANGEMENT).panel;
    expect(slotLength("panel", panel, 1600, 1000)).toBe("34%");
    expect(slotLength("panel", panel, 1600, 900)).toBe("34%");
    expect(slotLength("panel", panel, 1280, 800)).toBe("30%");
    // Dragged, it keeps what it was given whatever the window's height.
    const dragged = panel.map((one) => ({ ...one, size: 40 }));
    expect(slotLength("panel", dragged, 1280, 800)).toBe("40%");
  });

  it("measures the right panel's share against the row, not the window", () => {
    const right = inSlots(DEFAULT_ARRANGEMENT).right;
    // A 1530px window's row is 1530 less the two 44px bars (#3144): 300px is more than its 20%
    // there.
    expect(slotLength("right", right, rowWidth(1530))).toBe("20%");
    expect(slotLength("right", right, rowWidth(1600))).toBe("300px");
  });

  it("takes the project rail off the row where it is drawn (#2915)", () => {
    const right = inSlots(DEFAULT_ARRANGEMENT).right;
    const rail = document.createElement("div");
    rail.dataset.scope = "rail";
    rail.dataset.part = "slot";
    rail.getBoundingClientRect = () => ({ width: 52 }) as DOMRect;
    document.body.append(rail);
    try {
      expect(rowWidth(1600)).toBe(1600 - 44 - 44 - 52);
      // A 1600px window with the rail drawn: 300px is more than 20% of its 1460px row.
      expect(slotLength("right", right, rowWidth(1600))).toBe("20%");
    } finally {
      rail.remove();
    }
    // Without it the row is 1512px, and 300px fits in its 20%.
    expect(rowWidth(1600)).toBe(1600 - 44 - 44);
    expect(slotLength("right", right, rowWidth(1600))).toBe("300px");
  });

  it("takes the left bar's gutter off the row with the other two (#3144)", () => {
    document.documentElement.style.setProperty("--window-gutter", "8px");
    try {
      // The bars, a gutter at each end of the row and one and a half between the rail and the
      // left bar (D-3144-13).
      expect(rowWidth(1600)).toBe(1600 - 44 - 44 - 2 * 8 - 12);
    } finally {
      document.documentElement.style.removeProperty("--window-gutter");
    }
  });

  it("puts the side panel away on a pick of its view already open, and brings it back on the next (R121, #3329)", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("chats"));
    expect(navigation(result.current.arrangement).collapsed).toBe(true);

    act(() => result.current.pick("chats"));
    expect(navigation(result.current.arrangement)).toMatchObject({ collapsed: false });
    expect(openView(navigation(result.current.arrangement))).toBe("chats");
  });

  it("puts the right panel away on a pick of the view already open, and brings it back on the next", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    const aside = (arrangement: Arrangement) => {
      const found = arrangement.find((one) => one.id === "aside");
      if (found === undefined) throw new Error("no aside placed");
      return found;
    };

    act(() => result.current.pick("inbox"));
    expect(aside(result.current.arrangement).collapsed).toBe(true);

    act(() => result.current.pick("inbox"));
    expect(aside(result.current.arrangement)).toMatchObject({ collapsed: false });
    expect(openView(aside(result.current.arrangement))).toBe("inbox");
  });

  it("brings a side that is away back on the view picked", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.toggle("navigation"));

    act(() => result.current.pick("explorer"));

    expect(navigation(result.current.arrangement)).toMatchObject({
      view: "explorer",
      collapsed: false,
    });
  });

  it("is shown, never put away, when a key or the palette asks for it", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.show("chats"));
    expect(navigation(result.current.arrangement).collapsed).toBe(false);

    act(() => result.current.toggle("navigation"));
    act(() => result.current.show("explorer"));
    expect(navigation(result.current.arrangement)).toMatchObject({
      view: "explorer",
      collapsed: false,
    });
  });

  it("is remembered in the file", async () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("explorer"));

    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(lastWritten().projects["/one"].regions[0].view).toBe("explorer");
  });

  it("falls back on Chats for a view the side does not have, and says so", async () => {
    put({
      version: 2,
      regions: [{ id: "navigation", side: "left", order: 0, collapsed: false, view: "minimap" }],
    });

    expect(openView(navigation(remembered()))).toBe("chats");
    await settleLayout();
    expect(aboutThisMachine()[0].detail).toContain('"minimap"');
  });
});

describe("the Skills view, behind the Skill studio's preview (#3266)", () => {
  const navigation = navigationOf;
  const ON: readonly ViewId[] = ["skills"];

  it("sits under Search on the left's bar only while the preview is on", () => {
    expect(tabsOf(DEFAULT_ARRANGEMENT, "navigation", [])).toEqual(["chats", "explorer", "search"]);
    expect(tabsOf(DEFAULT_ARRANGEMENT, "navigation", ON)).toEqual([
      "chats",
      "explorer",
      "search",
      "skills",
    ]);
    // Never one of the right side's: what is present there is an extension's panel alone.
    expect(tabsOf(DEFAULT_ARRANGEMENT, "aside", ON)).not.toContain("skills");
  });

  it("falls back to Chats while the preview is off, and comes back when it is on again", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.pick("skills", ON));
    const picked = navigation(result.current.arrangement);
    expect(openView(picked, ON)).toBe("skills");

    // Off: the side opens on Chats, and the pick is kept rather than written over.
    expect(openView(picked, [])).toBe("chats");
    expect(picked.view).toBe("skills");
    // On again: the pick is back.
    expect(openView(picked, ON)).toBe("skills");
  });

  it("holds a side left on it until the window has heard whether the preview is on (#3308)", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.pick("skills", ON));
    const picked = navigation(result.current.arrangement);
    const views = result.current.arrangement.map((one) => one.view);
    // Not yet heard: the side stays on Skills, not Chats for a moment.
    expect(openView(picked, previewsDrawn(undefined, views))).toBe("skills");
    // A window left on Chats shows no Skills icon for a moment where the preview may be off.
    expect(
      previewsDrawn(
        undefined,
        DEFAULT_ARRANGEMENT.map((one) => one.view),
      ),
    ).toEqual([]);
    // Heard: on keeps it, off falls back to Chats.
    expect(openView(picked, previewsDrawn(true, views))).toBe("skills");
    expect(openView(picked, previewsDrawn(false, views))).toBe("chats");
  });

  it("is named for the palette only while it is drawn", () => {
    expect(placesOf(DEFAULT_ARRANGEMENT, []).map((one) => one.view)).not.toContain("skills");
    expect(placesOf(DEFAULT_ARRANGEMENT, [], ON)).toContainEqual(
      expect.objectContaining({ view: "skills", name: "Skills", region: "navigation" }),
    );
  });
});

describe("the open view of the right side (#1678)", () => {
  const aside = (arrangement: ReturnType<typeof remembered>) => {
    const found = placed(arrangement, "aside");
    if (found === undefined) throw new Error("no attention region");
    return found;
  };

  it("is the Inbox, first on its bar of Inbox, Personas and Vaults (#1738, P-2)", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    expect(CATALOGUE.aside.views).toEqual(["inbox", "personas", "vaults"]);
    expect(openView(aside(result.current.arrangement))).toBe("inbox");
  });

  it("puts the side away on a pick of the Inbox while it is open, and opens Personas on a pick of it", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("inbox"));
    expect(aside(result.current.arrangement).collapsed).toBe(true);

    act(() => result.current.pick("personas"));
    expect(aside(result.current.arrangement)).toMatchObject({ view: "personas", collapsed: false });
    // The left side is not touched by a pick on the right.
    expect(navigationOf(result.current.arrangement)).toEqual(DEFAULT_ARRANGEMENT[0]);
  });

  it("opens an extension's panel as a view of its own, and remembers it in the file", async () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("panel:ext/stats/burn", ["panel:ext/stats/burn"]));

    expect(openView(aside(result.current.arrangement), ["panel:ext/stats/burn"])).toBe(
      "panel:ext/stats/burn",
    );
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(lastWritten().projects["/one"].regions[1].view).toBe("panel:ext/stats/burn");
  });

  it("is read back from the file as an extension's panel, and opens on the Inbox while it is not there", () => {
    put({
      version: 2,
      regions: [{ id: "aside", side: "right", view: "panel:ext/stats/burn" }],
    });

    const remembers = aside(remembered());
    expect(remembers.view).toBe("panel:ext/stats/burn");
    expect(openView(remembers, ["panel:ext/stats/burn"])).toBe("panel:ext/stats/burn");
    // Its extension is not approved in this window: the side opens on its default.
    expect(openView(remembers, [])).toBe("inbox");
  });

  it("puts the side away on a pick of the view drawn open, when the remembered one is gone", () => {
    put({
      version: 2,
      regions: [{ id: "aside", side: "right", view: "panel:ext/gone/x" }],
    });
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("inbox", []));

    expect(aside(result.current.arrangement).collapsed).toBe(true);
  });

  it("keeps the side's width and whether it is away beside its view, under the project", async () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("vaults"));
    act(() => result.current.resized({ right: 28 }));
    act(() => result.current.pick("vaults"));

    await vi.waitFor(() => expect(sent).toHaveLength(3));
    expect(lastWritten().projects["/one"].regions[1]).toEqual({
      id: "aside",
      side: "right",
      order: 0,
      collapsed: true,
      size: 28,
      view: "vaults",
    });
    // And it comes back so for the project, read from the file the next launch is handed.
    put(lastWritten());
    expect(aside(remembered("/one"))).toMatchObject({ view: "vaults", size: 28, collapsed: true });
  });

  it("refuses a view that is neither the side's nor an extension panel's, and says so", async () => {
    put({ version: 2, regions: [{ id: "aside", side: "right", view: "chats" }] });

    expect(openView(aside(remembered()))).toBe("inbox");
    await settleLayout();
    expect(aboutThisMachine()[0].detail).toContain('"chats" is not a view of aside');
  });

  it("is not an extension's panel on the left, which takes none", async () => {
    put({
      version: 2,
      regions: [{ id: "navigation", side: "left", view: "panel:ext/stats/burn" }],
    });

    expect(navigationOf(remembered()).view).toBeUndefined();
  });
});

describe("Use the default layout (#1289), with each project's own (#1673)", () => {
  it("starts every project opened afterwards from the default, the file's and this launch's gone", async () => {
    put({
      version: 2,
      regions: [{ id: "navigation", side: "right", order: 0, collapsed: true }],
      projects: { "/one": { regions: [{ id: "navigation", side: "left", collapsed: true }] } },
    });
    const { result } = renderHook(() => useArrangement("/two"));
    act(() => result.current.pick("explorer"));

    act(() => usingTheDefaultLayout());

    expect(remembered("/one")).toEqual(DEFAULT_ARRANGEMENT);
    expect(remembered("/two")).toEqual(DEFAULT_ARRANGEMENT);
    expect(remembered("/three")).toEqual(DEFAULT_ARRANGEMENT);
  });
});

/**
 * **The Panel** (spec #1736, P-1 to P-14, #1737): a full-width region under both sidebars and the
 * editor, in a left half and a right half, each of its own side. Repos lives in the left half by
 * default; the right half holds nothing until #1738.
 */
describe("the Panel (#1737)", () => {
  const half = (arrangement: ReturnType<typeof remembered>, side: "left" | "right") => {
    const found = placed(arrangement, `panel-${side}`);
    if (found === undefined) throw new Error(`no ${side} half of the Panel`);
    return found;
  };

  it("is under both sidebars, its left half open on Repos, at 34% of the window (R126)", () => {
    const slots = inSlots(remembered());

    expect(slots.panel.map((one) => one.id)).toEqual(["panel-left", "panel-right"]);
    expect(slots.left.map((one) => one.id)).toEqual(["navigation"]);
    expect(slots.right.map((one) => one.id)).toEqual(["aside"]);
    expect(openView(half(remembered(), "left"))).toBe("repos");
    expect(panelShown(remembered())).toBe(true);
    expect(slotSize("panel", slots.panel)).toBe(34);
  });

  it("puts a half away on a press of its open Lower icon, and the Panel with it (P-5)", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("repos"));

    expect(half(result.current.arrangement, "left").collapsed).toBe(true);
    // The right half is still out (#1738), so the Panel is too, the right half alone in it.
    expect(shownIn(inSlots(result.current.arrangement).panel).map((one) => one.id)).toEqual([
      "panel-right",
    ]);
    act(() => result.current.pick("memory"));
    // Both away: the Panel is not drawn.
    expect(panelShown(result.current.arrangement)).toBe(false);
    // The sidebars are not the Panel's: both stay out.
    expect(placed(result.current.arrangement, "navigation")?.collapsed).toBe(false);
    expect(placed(result.current.arrangement, "aside")?.collapsed).toBe(false);

    act(() => result.current.pick("repos"));

    expect(panelShown(result.current.arrangement)).toBe(true);
  });

  it("is shown and hidden whole by ⌘J, and a hidden one comes back on its halves", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.toggle(PANEL));

    expect(panelShown(result.current.arrangement)).toBe(false);
    expect(placed(result.current.arrangement, "navigation")?.collapsed).toBe(false);

    act(() => result.current.toggle(PANEL));

    expect(half(result.current.arrangement, "left").collapsed).toBe(false);
    expect(panelShown(result.current.arrangement)).toBe(true);
  });

  it("is shown by ⌘J after both halves were put away by their icons", () => {
    const { result } = renderHook(() => useArrangement());
    act(() => result.current.pick("repos"));
    act(() => result.current.pick("memory"));
    expect(panelShown(result.current.arrangement)).toBe(false);

    act(() => result.current.toggle(PANEL));

    expect(openView(half(result.current.arrangement, "left"))).toBe("repos");
    expect(panelShown(result.current.arrangement)).toBe(true);
  });

  it("keeps the height it was dragged to, for the project, and draws it from the file", async () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.resized({ panel: 42 }));

    expect(slotSize("panel", inSlots(result.current.arrangement).panel)).toBe(42);
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(lastWritten().projects["/one"].regions).toContainEqual({
      id: "panel-left",
      side: "left",
      order: 0,
      collapsed: false,
      size: 42,
    });
    // The next launch, handed that file.
    forgetThisLaunch();
    put(lastWritten());
    expect(slotSize("panel", inSlots(remembered("/one")).panel)).toBe(42);
  });

  it("is never taller than 70% of the window, whatever the file says (P-12)", () => {
    put({
      version: 2,
      regions: [{ id: "panel-left", side: "left", order: 0, collapsed: false, size: 95 }],
    });

    expect(slotSize("panel", inSlots(remembered()).panel)).toBe(70);
    // In pixels too: the floor is the slot's, whatever the window's height (P-12).
    expect(leastOf("panel")).toBe("160px");
  });

  it("goes back to 34% on a double-click of its handle, and remembers that", async () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.resized({ panel: 55 }));

    act(() => result.current.reset("panel"));

    expect(slotSize("panel", inSlots(result.current.arrangement).panel)).toBe(34);
    await vi.waitFor(() => expect(sent).toHaveLength(2));
    expect(
      lastWritten().projects["/one"].regions.find((one: { id: string }) => one.id === "panel-left"),
    ).not.toHaveProperty("size");
  });

  it("keeps a half's height and whether it is away from a file, and its side for good", () => {
    put({
      version: 2,
      regions: [
        { id: "panel-left", side: "right", order: 0, collapsed: true, size: 40, view: "repos" },
      ],
    });

    expect(half(remembered(), "left")).toEqual({
      id: "panel-left",
      side: "left",
      order: 0,
      collapsed: true,
      size: 40,
      view: "repos",
    });
    // The right half is the file's to leave out, so it is drawn alone (#1738).
    expect(shownIn(inSlots(remembered()).panel).map((one) => one.id)).toEqual(["panel-right"]);
  });

  it("opens once after the update, from a file that never placed it, then stays as left (P-14)", async () => {
    // The file every build before #1737 wrote: the two sides and nothing of the Panel.
    put({
      version: 2,
      regions: [{ id: "navigation", side: "left", order: 0, collapsed: false }],
      projects: { "/one": { regions: [{ id: "aside", side: "right", order: 0, view: "todos" }] } },
    });
    expect(panelShown(remembered("/one"))).toBe(true);
    expect(slotSize("panel", inSlots(remembered("/one")).panel)).toBe(34);
    await settleLayout();
    // Nothing to say: a region a file does not place is drawn where it starts.
    expect(aboutThisMachine()).toEqual([]);

    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.toggle(PANEL));
    await vi.waitFor(() => expect(sent).toHaveLength(1));

    // The next launch, handed what that wrote: the Panel stays put away.
    forgetThisLaunch();
    put(lastWritten());
    expect(panelShown(remembered("/one"))).toBe(false);
  });

  it("comes back open at its default height on Use the default layout", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.resized({ panel: 60 }));
    act(() => result.current.toggle(PANEL));

    act(() => usingTheDefaultLayout());

    expect(panelShown(remembered("/one"))).toBe(true);
    expect(slotSize("panel", inSlots(remembered("/one")).panel)).toBe(34);
  });

  it("holds Changes in its left half, after Repos, and the left sidebar keeps Chats, Explorer, Search and Skills (#1739, P-2, #3266)", () => {
    expect(CATALOGUE.navigation.views).toEqual(["chats", "explorer", "search", "skills"]);
    expect(CATALOGUE["panel-left"].views).toEqual(["repos", "changes"]);
    expect(regionOf("changes")).toBe("panel-left");
    // Repos is still what the half opens on (P-14).
    expect(openView(half(remembered(), "left"))).toBe("repos");
  });

  it("opens Changes in its half on its key, a hidden Panel too, and leaves the sidebar alone (P-6)", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.pick("explorer"));
    act(() => result.current.toggle(PANEL));

    act(() => result.current.show("changes"));

    expect(half(result.current.arrangement, "left")).toMatchObject({
      view: "changes",
      collapsed: false,
    });
    expect(panelShown(result.current.arrangement)).toBe(true);
    expect(navigationOf(result.current.arrangement)).toMatchObject({
      view: "explorer",
      collapsed: false,
    });
  });

  it("opens a sidebar's view in the sidebar on its key, and leaves the Panel as it was", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.toggle(PANEL));
    act(() => result.current.toggle("navigation"));

    act(() => result.current.show("search"));

    expect(navigationOf(result.current.arrangement)).toMatchObject({
      view: "search",
      collapsed: false,
    });
    expect(panelShown(result.current.arrangement)).toBe(false);
  });

  it("reads a sidebar left open on Changes as open on Chats, without a word: Changes moved down", async () => {
    // What every build before #1739 wrote for a person who had Changes open on the left.
    put({
      version: 2,
      regions: [{ id: "navigation", side: "left", order: 0, collapsed: false, view: "changes" }],
    });

    expect(openView(navigationOf(remembered()))).toBe("chats");
    expect(openView(half(remembered(), "left"))).toBe("repos");
    await settleLayout();
    expect(aboutThisMachine()).toEqual([]);
  });

  it("still says a view saved on the other side's region is not that region's (P-3)", async () => {
    // Memory is the right side's; a view never moves across sides, so this is a fault.
    put({
      version: 2,
      regions: [{ id: "navigation", side: "left", order: 0, collapsed: false, view: "memory" }],
    });

    expect(openView(navigationOf(remembered()))).toBe("chats");
    await settleLayout();
    expect(JSON.stringify(aboutThisMachine())).toContain("memory");
  });

  it("keeps every project's Panel inside the projects' budget in the file", async () => {
    // Every project the file can hold, each arranged with its Panel moved: all are sent.
    for (let at = 0; at < MOST_PROJECTS; at += 1) {
      const { result, unmount } = renderHook(() =>
        useArrangement(`/home/someone/projects/a-project-with-a-long-name-${at}`),
      );
      act(() => result.current.resized({ panel: 41.25, left: 22.5, right: 27.75 }));
      act(() => result.current.pick("todos"));
      unmount();
    }

    await vi.waitFor(() => expect(sent).toHaveLength(MOST_PROJECTS * 2));
    const projects = lastWritten().projects as Record<string, unknown>;
    expect(Object.keys(projects)).toHaveLength(MOST_PROJECTS);
    const bytes = new TextEncoder().encode(JSON.stringify(projects, null, 2)).length;
    expect(bytes).toBeLessThanOrEqual(PROJECTS_MOST_BYTES);
  });
});

/**
 * **The Panel's right half** (#1738, P-2, P-5, P-12, P-14): Memory, Todos and Sessions, out of the
 * right sidebar, which keeps the Inbox, Personas and Vaults; and the handle between the halves.
 */
describe("the Panel's right half (#1738)", () => {
  const half = (arrangement: ReturnType<typeof remembered>, side: "left" | "right") => {
    const found = placed(arrangement, `panel-${side}`);
    if (found === undefined) throw new Error(`no ${side} half of the Panel`);
    return found;
  };

  it("holds Memory, Todos and Sessions, and opens on Memory beside Repos (P-2, P-14)", () => {
    expect(CATALOGUE["panel-right"].views).toEqual(["memory", "todos", "sessions"]);
    expect(openView(half(remembered(), "left"))).toBe("repos");
    expect(openView(half(remembered(), "right"))).toBe("memory");
    expect(shownIn(inSlots(remembered()).panel).map((one) => one.id)).toEqual([
      "panel-left",
      "panel-right",
    ]);
  });

  it("is where a key or the palette opens Memory, Todos and Sessions, never the sidebar", () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.pick("memory"));
    expect(half(result.current.arrangement, "right").collapsed).toBe(true);

    act(() => result.current.show("sessions"));

    expect(half(result.current.arrangement, "right")).toMatchObject({
      view: "sessions",
      collapsed: false,
    });
    expect(placed(result.current.arrangement, "aside")).toMatchObject({ collapsed: false });
    expect(placed(result.current.arrangement, "aside")?.view).toBeUndefined();
  });

  it("gives the left half the Panel's whole width while the right is away, and back (P-5)", () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.pick("memory"));

    expect(shownIn(inSlots(result.current.arrangement).panel).map((one) => one.id)).toEqual([
      "panel-left",
    ]);
    expect(halvesSplit(result.current.arrangement)).toEqual({ left: 100, right: 0 });

    act(() => result.current.pick("repos"));
    act(() => result.current.pick("todos"));

    expect(halvesSplit(result.current.arrangement)).toEqual({ left: 0, right: 100 });
  });

  it("splits the Panel's width 50/50 until the handle between the halves is dragged (P-12)", () => {
    expect(halvesSplit(remembered())).toEqual({ left: 50, right: 50 });
  });

  it("keeps the split it was dragged to, for the project, and draws it from the file", async () => {
    const { result } = renderHook(() => useArrangement("/one"));

    act(() => result.current.split(35));

    expect(halvesSplit(result.current.arrangement)).toEqual({ left: 35, right: 65 });
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    // The left half's share of the width, on the left half: the one number the split is.
    expect(
      lastWritten().projects["/one"].regions.find((one: { id: string }) => one.id === "panel-left"),
    ).toMatchObject({ share: 35 });
    // The next launch, handed that file.
    forgetThisLaunch();
    put(lastWritten());
    expect(halvesSplit(remembered("/one"))).toEqual({ left: 35, right: 65 });
  });

  it("goes back to 50/50 on a double-click of the handle, and remembers that (P-12)", async () => {
    const { result } = renderHook(() => useArrangement("/one"));
    act(() => result.current.split(70));

    act(() => result.current.reset(HALVES));

    expect(halvesSplit(result.current.arrangement)).toEqual({ left: 50, right: 50 });
    await vi.waitFor(() => expect(sent).toHaveLength(2));
    expect(
      lastWritten().projects["/one"].regions.find((one: { id: string }) => one.id === "panel-left"),
    ).not.toHaveProperty("share");
  });

  it("never lets either half be squeezed below a fifth of the width, whatever the file says", () => {
    put({
      version: 2,
      regions: [{ id: "panel-left", side: "left", order: 0, collapsed: false, share: 97 }],
    });
    expect(halvesSplit(remembered())).toEqual({ left: 80, right: 20 });

    put({
      version: 2,
      regions: [{ id: "panel-left", side: "left", order: 0, collapsed: false, share: "half" }],
    });
    expect(halvesSplit(remembered())).toEqual({ left: 50, right: 50 });
  });

  it("is read without a word from a file whose right sidebar was left on Memory, Todos or Sessions", async () => {
    // Every build before #1738 kept the right sidebar's open view, Memory by default.
    put({
      version: 2,
      regions: [{ id: "aside", side: "right", order: 0, collapsed: false, view: "memory" }],
      projects: {
        "/one": { regions: [{ id: "aside", side: "right", order: 0, view: "sessions" }] },
      },
    });

    expect(placed(remembered(), "aside")).not.toHaveProperty("view");
    expect(placed(remembered("/one"), "aside")).not.toHaveProperty("view");
    // The first launch after the update shows Memory in the right half (P-14).
    expect(openView(half(remembered("/one"), "right"))).toBe("memory");
    await settleLayout();
    expect(aboutThisMachine()).toEqual([]);
  });
});
