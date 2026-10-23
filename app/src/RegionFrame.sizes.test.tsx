// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef, useState, type ReactNode, type Ref } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { RegionFrame } from "./RegionFrame";
import { DEFAULT_ARRANGEMENT, type Arrangement, type RegionId, type Side } from "./regions";

/**
 * **What the frame tells `react-resizable-panels`.**
 *
 * The library is mocked here, and that is not a shortcut around it — jsdom gives every element
 * a size of zero, so the real library defers its layout and never applies a `defaultSize` at
 * all. Reading a size off the DOM in a unit test is therefore impossible, and the question this
 * file asks is the one that *is* answerable: what was the library told, and when. `RegionFrame`
 * runs against the real library in `RegionFrame.test.tsx`, and the whole window does in
 * `FourRegions.test.tsx`.
 */

type PanelProps = {
  id: string;
  className?: string;
  collapsible?: boolean;
  collapsedSize?: string;
  defaultSize?: string;
  minSize?: string;
  maxSize?: string;
  panelRef?: Ref<unknown>;
  children?: ReactNode;
};

type Handle = {
  collapse: ReturnType<typeof vi.fn>;
  expand: ReturnType<typeof vi.fn>;
  resize: ReturnType<typeof vi.fn>;
};

/** Every `Panel` the frame rendered, by id, as it was first given — the constraints a group is
 *  registered with are the ones that matter, so the FIRST render's props are kept as well. */
const given = new Map<string, PanelProps>();
const first = new Map<string, PanelProps>();
const handles = new Map<string, Handle>();
/** Every `Group`'s settled-layout callback, in the order the frame rendered them: the one it
 *  was last given, as the library calls the prop it has now. */
const settled: ((layout: Record<string, number>, meta: { isUserInteraction: boolean }) => void)[] =
  [];
/** Which group each of {@link settled} is, by its class. */
const settledOf: (string | undefined)[] = [];

vi.mock("react-resizable-panels", () => ({
  Group: ({
    children,
    className,
    onLayoutChanged,
  }: {
    children?: ReactNode;
    className?: string;
    onLayoutChanged?: (l: Record<string, number>, m: { isUserInteraction: boolean }) => void;
  }) => {
    if (onLayoutChanged) {
      const at = settledOf.indexOf(className);
      if (at < 0) {
        settledOf.push(className);
        settled.push(onLayoutChanged);
      } else settled[at] = onLayoutChanged;
    }
    return <div data-group={className}>{children}</div>;
  },
  Panel: (props: PanelProps) => {
    given.set(props.id, props);
    if (!first.has(props.id)) first.set(props.id, props);
    const handle = useRef<Handle>({ collapse: vi.fn(), expand: vi.fn(), resize: vi.fn() });
    const { panelRef } = props;
    // A layout effect, because that is when the real library attaches its imperative handle —
    // before the parent's passive effect reaches for it.
    useLayoutEffect(() => {
      if (typeof panelRef === "object" && panelRef !== null) panelRef.current = handle.current;
      handles.set(props.id, handle.current);
    });
    return (
      <div data-panel={props.id} data-testid={props.id}>
        {props.children}
      </div>
    );
  },
  Separator: ({
    className,
    elementRef,
    disableDoubleClick,
  }: {
    className?: string;
    elementRef?: Ref<HTMLDivElement>;
    disableDoubleClick?: boolean;
  }) => (
    <div
      role="separator"
      ref={elementRef}
      className={className}
      data-own-reset={disableDoubleClick ? "" : undefined}
    />
  ),
  usePanelRef: () => useRef(null),
}));

const CONTENT: Partial<Record<RegionId, ReactNode>> = {
  navigation: <div data-testid="c-navigation">navigation</div>,
  aside: <div data-testid="c-aside">aside</div>,
};

const onResized = vi.fn();
const onReset = vi.fn();
const onSplit = vi.fn();

function Harness({ from, to }: { from: Arrangement; to?: Arrangement }) {
  const [arrangement, setArrangement] = useState(from);
  return (
    <>
      <button onClick={() => to !== undefined && setArrangement(to)}>rearrange</button>
      <RegionFrame
        arrangement={arrangement}
        content={CONTENT}
        centre={<div data-testid="centre">centre</div>}
        onResized={onResized}
        onReset={onReset}
        onSplit={onSplit}
      />
    </>
  );
}

const rearrange = () => userEvent.click(screen.getByRole("button", { name: "rearrange" }));

const on = (id: RegionId, side: Side, order = 0, collapsed = false, size?: number) => ({
  id,
  side,
  order,
  collapsed,
  ...(size === undefined ? {} : { size }),
});

const AWAY: Arrangement = [on("navigation", "left", 0, true), on("aside", "right")];

beforeEach(() => {
  given.clear();
  first.clear();
  handles.clear();
  settled.length = 0;
  settledOf.length = 0;
  onResized.mockClear();
  onReset.mockClear();
  onSplit.mockClear();
});
afterEach(cleanup);

describe("how big a slot starts", () => {
  it("is the catalogue's when nothing has been dragged", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    expect(given.get("region-left")?.defaultSize).toBe("16%");
    // The right panel is 300px until it is dragged (R124), never more than its 20%: jsdom's
    // window is 1024px wide.
    expect(given.get("region-right")?.defaultSize).toBe("20%");
    // Nothing along the bottom since #1676: the panes are the window's whole height.
    expect(given.has("region-bottom")).toBe(false);
  });

  it("is the size the arrangement remembers, which is what charter-app#141 deferred", () => {
    render(<Harness from={[on("navigation", "left", 0, false, 31.5), on("aside", "right")]} />);

    expect(given.get("region-left")?.defaultSize).toBe("31.5%");
  });

  it("is nothing at all when the slot starts with nothing in it", () => {
    // **The flash** (charter-app#141, deliberate at the time): a region that was put away used
    // to be laid out full size and collapsed by an effect, and an effect runs after the browser
    // has painted. Starting the slot at `0%` — which the library snaps to `collapsedSize` — is
    // what makes the first frame right, because `useLayoutEffect` cannot: the group registers
    // itself in its own layout effect, which React runs AFTER its children's.
    render(<Harness from={AWAY} />);

    expect(given.get("region-left")?.defaultSize).toBe("0%");
    expect(given.get("region-left")?.collapsedSize).toBe("0%");
    expect(given.get("region-left")?.collapsible).toBe(true);
  });

  it("is nothing at all when every region has moved off that side", () => {
    render(<Harness from={[on("navigation", "right", 1), on("aside", "right")]} />);

    expect(given.get("region-left")?.defaultSize).toBe("0%");
  });
});

describe("the constraints the group is registered with", () => {
  it("never change, however the arrangement does", async () => {
    // charter-app#141 measured both halves of this: taking a panel out of a live group throws,
    // and so does changing a live panel's constraints, because the change re-registers it and a
    // separator recalculating in the gap indexes past the end of the constraint list.
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "right", 1, true), on("aside", "left", 0, false, 40)]}
      />,
    );

    await rearrange();

    for (const [id, was] of first) {
      expect({ id, ...constraints(given.get(id)) }).toEqual({ id, ...constraints(was) });
    }
  });

  it("gives each slot the bounds of the slot and not of whatever is in it", () => {
    // A bound taken from the regions placed in a slot would move the moment one of them did,
    // which is the throw above. `SLOTS` is where they are written, once.
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    // The left slot's floor is in the text's own unit (#1499): 8% of a narrow window leaves a
    // nested row of the Chats list no room for its state.
    expect(constraints(given.get("region-left"))).toMatchObject({
      minSize: "11rem",
      maxSize: "45%",
    });
    expect(constraints(given.get("region-right"))).toMatchObject({
      minSize: "10%",
      maxSize: "45%",
    });
  });
});

const constraints = (props: PanelProps | undefined) => ({
  collapsedSize: props?.collapsedSize,
  collapsible: props?.collapsible,
  defaultSize: props?.defaultSize,
  minSize: props?.minSize,
  maxSize: props?.maxSize,
});

describe("putting a region away while the window is up", () => {
  it("touches nothing on the launch itself", () => {
    // `defaultSize` has already laid the slots out. A collapse or a resize here would be a
    // second layout for no change, landing while the group is still measuring itself.
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    for (const handle of handles.values()) {
      expect(handle.expand).not.toHaveBeenCalled();
      expect(handle.resize).not.toHaveBeenCalled();
    }
  });

  it("collapses the slot the last region left", async () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} to={AWAY} />);

    await rearrange();

    expect(handles.get("region-left")?.collapse).toHaveBeenCalled();
    expect(handles.get("region-right")?.collapse).not.toHaveBeenCalled();
  });

  it("brings it back at the size it was left, and not at the library's minimum", async () => {
    // `expand()` restores the size the panel had when it was collapsed, and a slot that was
    // already away at launch never had one — it started at nothing, so the library would bring
    // it back at `minSize`. The remembered size is what it should come back to.
    render(
      <Harness
        from={[on("navigation", "left", 0, true, 31.5), on("aside", "right")]}
        to={[on("navigation", "left", 0, false, 31.5), on("aside", "right")]}
      />,
    );

    await rearrange();

    expect(handles.get("region-left")?.expand).toHaveBeenCalled();
    expect(handles.get("region-left")?.resize).toHaveBeenCalledWith("31.5%");
  });
});

describe("remembering how big a slot was left", () => {
  const layout = { "region-left": 31.5, "region-centre": 48.5, "region-right": 20 };

  it("reports each slot under the id its panel is drawn with", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    settled[0]?.(layout, { isUserInteraction: true });

    expect(onResized).toHaveBeenCalledWith({ left: 31.5, right: 20 });
  });

  it("says nothing about a slot the group did not report", () => {
    // A layout that does not mention the left slot leaves its width alone, rather than writing
    // a width it was never told.
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    settled[0]?.({ "region-centre": 80, "region-right": 20 }, { isUserInteraction: true });

    expect(onResized).toHaveBeenCalledWith({ right: 20 });
  });

  it("ignores a layout charter itself caused", () => {
    // `onLayoutChanged` also fires on mount, on a collapse and on every imperative call —
    // including the zero a collapsed slot reports, which is not a width to come back to.
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    settled[0]?.(layout, { isUserInteraction: false });

    expect(onResized).not.toHaveBeenCalled();
  });
});

/** The Panel's slot (#1737, P-12): 30% of the window by default, 120px to 70%. */
describe("the Panel's height", () => {
  it("starts at its share, held between 160px and 70% (R126), and collapses to nothing", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    expect(constraints(given.get("region-panel"))).toEqual({
      collapsedSize: "0%",
      collapsible: true,
      defaultSize: "34%",
      minSize: "160px",
      maxSize: "70%",
    });
  });

  it("starts at nothing when every half of it was put away", () => {
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT.map((one) =>
          one.id.startsWith("panel-") ? { ...one, collapsed: true } : one,
        )}
      />,
    );

    expect(given.get("region-panel")?.defaultSize).toBe("0%");
  });

  it("is reported as the Panel's, beside the sides' widths", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    for (const report of settled)
      report(
        { "region-upper": 58, "region-panel": 42 },
        {
          isUserInteraction: true,
        },
      );

    expect(onResized).toHaveBeenCalledWith({ panel: 42 });
  });

  it("goes back to its default on a double-click of its handle, charter's and not the library's", async () => {
    render(
      <Harness
        from={[...DEFAULT_ARRANGEMENT.slice(0, 2), { ...on("panel-left", "left"), size: 55 }]}
      />,
    );
    const handle = drawnAt(panelHandle(), LINE);

    await userEvent.dblClick(handle);

    expect(handle).toHaveAttribute("data-own-reset");
    expect(handles.get("region-panel")?.resize).toHaveBeenCalledWith("34%");
    expect(onReset).toHaveBeenCalledWith("panel");
  });

  it("goes back on two quick presses of its handle too, where the window sends no dblclick", () => {
    render(
      <Harness
        from={[...DEFAULT_ARRANGEMENT.slice(0, 2), { ...on("panel-left", "left"), size: 55 }]}
      />,
    );
    const handle = drawnAt(panelHandle(), LINE);

    pressAt(handle, 1000);
    expect(onReset).not.toHaveBeenCalled();
    pressAt(handle, 1200);

    expect(handles.get("region-panel")?.resize).toHaveBeenCalledWith("34%");
    expect(onReset).toHaveBeenCalledWith("panel");
  });

  it("counts presses beside the 1px handle, in the band the library takes, and not past it", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);
    drawnAt(panelHandle(), DOMRect.fromRect({ x: 40, y: 500, width: 900, height: 1 }));
    const panel = screen.getByTestId("region-panel");

    // 4px under the line, on the Panel's own content: the library's band, so a double-click.
    pressAt(panel, 1000, { clientX: 400, clientY: 505 });
    pressAt(panel, 1100, { clientX: 400, clientY: 505 });
    expect(onReset).toHaveBeenCalledTimes(1);

    // 20px under it is the Panel, not the handle.
    pressAt(panel, 2000, { clientX: 400, clientY: 521 });
    pressAt(panel, 2100, { clientX: 400, clientY: 521 });
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it("goes back on two mouse presses with no pointer events, as the real window's WebDriver sends", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);
    const handle = drawnAt(panelHandle(), LINE);

    mouseAt(handle, 1000);
    mouseAt(handle, 1100);

    expect(onReset).toHaveBeenCalledWith("panel");
  });

  it("counts a press's pointerdown and its mousedown as one press", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);
    const handle = drawnAt(panelHandle(), LINE);

    pressAt(handle, 1000);
    mouseAt(handle, 1010);

    expect(onReset).not.toHaveBeenCalled();
  });

  it("is never pressed while it has no box, as a handle put away has not", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);
    // jsdom lays nothing out: every box is empty, at the window's corner.
    pressAt(panelHandle(), 1000);
    pressAt(panelHandle(), 1100);
    fireEvent.dblClick(panelHandle());

    expect(onReset).not.toHaveBeenCalled();
  });

  it("is not reset by two presses far apart, which are two drags", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);
    const handle = drawnAt(panelHandle(), LINE);

    pressAt(handle, 1000);
    pressAt(handle, 3000);

    expect(onReset).not.toHaveBeenCalled();
  });
});

/** A press on `target` at `at` ms on the window's clock, at the handle unless `where` says. */
function pressAt(
  target: HTMLElement,
  at: number,
  where: { clientX: number; clientY: number } = { clientX: 0, clientY: 0 },
) {
  const clock = vi.spyOn(performance, "now").mockReturnValue(at);
  fireEvent.pointerDown(target, { pointerType: "mouse", button: 0, ...where });
  clock.mockRestore();
}

/** The handle between the Panel's halves (#1738, P-12): half each, a fifth at least, kept. */
describe("how the Panel's halves share its width", () => {
  const halves = (arrangement: Arrangement) =>
    arrangement.map((one) => (one.id === "panel-left" ? { ...one, share: 35 } : one));

  it("starts half each, neither under a fifth, either collapsible to nothing", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    for (const side of ["left", "right"]) {
      expect(constraints(given.get(`panel-half-${side}`))).toEqual({
        collapsedSize: "0%",
        collapsible: true,
        defaultSize: "50%",
        minSize: "20%",
        maxSize: "100%",
      });
    }
  });

  it("starts at the share the project kept", () => {
    render(<Harness from={halves(DEFAULT_ARRANGEMENT)} />);

    expect(given.get("panel-half-left")?.defaultSize).toBe("35%");
    expect(given.get("panel-half-right")?.defaultSize).toBe("65%");
  });

  it("starts the half that is out at the whole width, and the one put away at nothing", () => {
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT.map((one) =>
          one.id === "panel-right" ? { ...one, collapsed: true } : one,
        )}
      />,
    );

    expect(given.get("panel-half-left")?.defaultSize).toBe("100%");
    expect(given.get("panel-half-right")?.defaultSize).toBe("0%");
  });

  it("is reported as the left half's share once a drag settles, and nothing else is", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    for (const report of settled)
      report({ "panel-half-left": 40, "panel-half-right": 60 }, { isUserInteraction: true });
    for (const report of settled)
      report({ "panel-half-left": 45, "panel-half-right": 55 }, { isUserInteraction: false });

    expect(onSplit.mock.calls).toEqual([[40]]);
    expect(onResized).not.toHaveBeenCalled();
  });

  it("goes back to half each on a double-click of its handle", async () => {
    render(<Harness from={halves(DEFAULT_ARRANGEMENT)} />);
    const handle = drawnAt(
      screen.getByTestId("region-panel").querySelector('[role="separator"]') as HTMLElement,
      DOMRect.fromRect({ x: 0, y: 0, width: 1, height: 200 }),
    );

    await userEvent.dblClick(handle);

    expect(handle).toHaveAttribute("data-own-reset");
    expect(handles.get("panel-half-left")?.resize).toHaveBeenCalledWith("50%");
    expect(onReset).toHaveBeenCalledWith("halves");
  });

  it("collapses a half put away while the window is up, and gives it its share back", async () => {
    const away = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "panel-left" ? { ...one, collapsed: true } : one,
    );
    render(<Harness from={halves(DEFAULT_ARRANGEMENT)} to={away} />);

    await rearrange();

    expect(handles.get("panel-half-left")?.collapse).toHaveBeenCalled();
    expect(handles.get("panel-half-right")?.collapse).not.toHaveBeenCalled();
  });

  it("brings a half back at its share", async () => {
    const away = halves(DEFAULT_ARRANGEMENT).map((one) =>
      one.id === "panel-left" ? { ...one, collapsed: true } : one,
    );
    render(<Harness from={away} to={halves(DEFAULT_ARRANGEMENT)} />);

    await rearrange();

    expect(handles.get("panel-half-left")?.expand).toHaveBeenCalled();
    expect(handles.get("panel-half-left")?.resize).toHaveBeenCalledWith("35%");
  });
});

/** The handle between the editor and the Panel: the last one outside the Panel. */
function panelHandle(): HTMLElement {
  return [...document.querySelectorAll<HTMLElement>('[role="separator"]')]
    .filter((one) => !screen.getByTestId("region-panel").contains(one))
    .at(-1) as HTMLElement;
}

/** A 1px line across the window's top-left corner, where a press with no coordinates lands. */
const LINE = DOMRect.fromRect({ x: 0, y: 0, width: 900, height: 1 });

/** `handle`, laid out at `box`, as jsdom never lays anything out. */
function drawnAt(handle: HTMLElement, box: DOMRect): HTMLElement {
  vi.spyOn(handle, "getBoundingClientRect").mockReturnValue(box);
  return handle;
}

/** A `mousedown` alone on `target` at `at` ms, as a press with no pointer events is. */
function mouseAt(target: HTMLElement, at: number) {
  const clock = vi.spyOn(performance, "now").mockReturnValue(at);
  fireEvent.mouseDown(target, { button: 0, clientX: 0, clientY: 0 });
  clock.mockRestore();
}
