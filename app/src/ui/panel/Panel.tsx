// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The panel family and its sections** (#2913, spec #2897, ADR 0104): every region panel of the
 * window (a side panel, the content, a half of the bottom panel) and every section a panel holds,
 * drawn from one wrapper each. A screen imports these and never writes a panel's or a section
 * heading's look itself (`purlis/panel-family` in `eslint.config.js`).
 *
 * - **A panel is drawn by its root.** `data-scope="panel"`, `data-part="root"` and its
 *   `data-variant` (`side`, `content`, `bottom`). `panel.css` draws its ground and its corners
 *   from the theme's `panel.corner-radius`; the gap between panels is the theme's
 *   `window.gutter`, which the window's frame draws (`App.css`, the regions). Purlis Classic's
 *   corners and gutter are 0, so its panels sit flush with the hairline between them as before.
 * - **A resizable region is a library panel**, whose own props reach its outer box: such a
 *   region takes {@link panelRoot}'s attributes, and its inner box follows the root's corners.
 *   Any other panel is a {@link Panel}.
 * - **A section** is its root, its heading line (`head`) and its title (`title`), each a part.
 *   The line between two sections is a separator, never a section's border.
 * - **A view** is a section with a top (#3340, the sidebar-views prototype's Q1 A):
 *   the one thing a sidebar or a half of the bottom panel shows. It is ONE scroll; its `top`
 *   (its heading line, then its field, then its tabs and chips) is held in place on the panel's
 *   own fill while its rows scroll under it, and each group's name is held under the top while
 *   its group is in sight. A list inside it puts its search field in the top with
 *   {@link InPanelTop}.
 */
import clsx from "clsx";
import {
  createContext,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentPropsWithRef,
  type ComponentType,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

/** The panel's variants, in the order the manifest names them. */
export const PANEL_VARIANTS = ["side", "content", "bottom"] as const;
export type PanelVariant = (typeof PANEL_VARIANTS)[number];

/** The attributes a panel's root carries, for a region drawn by a library's panel. */
export function panelRoot(variant: PanelVariant) {
  return { "data-scope": "panel", "data-part": "root", "data-variant": variant } as const;
}

/** **A panel**: a region of the window that is not a library's resizable panel. */
export function Panel({
  variant,
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style"> & { variant: PanelVariant }) {
  return (
    <div {...rest} {...panelRoot(variant)}>
      {children}
    </div>
  );
}

/**
 * **A section of a panel**: its heading line, then what it holds.
 *
 * As a `view` it is the whole of what a sidebar or a half shows: `top` is drawn first, in the
 * section's `top` part, held at the top of the one scroll, and every list in `children` can put
 * its field there ({@link InPanelTop}). The section scrolls nothing itself: the region it is in
 * is the one scroll. A view is a section with a top (`data-frame`), not a variant of its own: its
 * heading and title are the region's ({@link PanelTitle}), so a view adds the one part.
 */
export function PanelSection({
  view = false,
  top,
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"section">, "style"> & {
  view?: boolean;
  /** A view's top, before its lists' fields: its heading line, its tabs and chips. */
  top?: ReactNode;
  [data: `data-${string}`]: string | undefined;
}) {
  if (view) {
    return (
      <section
        {...rest}
        data-scope="panel-section"
        data-part="root"
        data-variant="default"
        data-frame=""
      >
        <ViewTop top={top}>{children}</ViewTop>
      </section>
    );
  }
  return (
    <section {...rest} data-scope="panel-section" data-part="root" data-variant="default">
      {children}
    </section>
  );
}

/**
 * **The title of the view a panel shows** (#3340, the sidebar-views prototype's Q1 A): its one
 * heading, on the region's head (a half's line, a sidebar's head) or on the view's own heading
 * line where it is drawn on its own. 13px, strong, in the primary tier and its own case: the
 * panel family's `title` part, so a theme reaches every view's title through one key.
 */
export function PanelTitle({
  className,
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"h2">, "style">) {
  return (
    <h2
      {...rest}
      className={clsx("ui-text-body ui-text-strong", className)}
      data-scope="panel"
      data-part="title"
    >
      {children}
    </h2>
  );
}

/**
 * **Where a list in a view puts its field**: the view's top, after what the view put there.
 * `undefined` outside a view, `null` while the top is not mounted yet.
 */
const TopSlot = createContext<HTMLElement | null | undefined>(undefined);

/** The custom property a view's top writes its height to, which its groups' names stick under. */
export const TOP_SIZE = "--panel-section-top-size";

/**
 * A view's top and the slot its lists' fields go into, with the lists after it: one component,
 * so the slot a list asks for is always this view's.
 */
function ViewTop({ top: held, children }: { top: ReactNode; children: ReactNode }) {
  const top = useRef<HTMLDivElement>(null);
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  // **The groups' names stick under the top, however tall it is**: its height is the view's own
  // custom property, measured whenever it changes (a field's error line, chips that wrap).
  useLayoutEffect(() => {
    const it = top.current;
    const view = it?.parentElement;
    if (!it || !view) return;
    const write = () => view.style.setProperty(TOP_SIZE, `${it.offsetHeight}px`);
    write();
    if (typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(write);
    watch.observe(it);
    return () => watch.disconnect();
  }, []);
  return (
    <>
      <div ref={top} data-scope="panel-section" data-part="top">
        {held}
        <div ref={setSlot} data-top-slot="" />
      </div>
      <TopSlot.Provider value={slot}>{children}</TopSlot.Provider>
    </>
  );
}

/** Whether what is drawn here is inside a view (#3340): a list draws its groups' counts there. */
export function useInPanelView(): boolean {
  return useContext(TopSlot) !== undefined;
}

/**
 * **What a list puts in its view's top** (#3340): its search field, so the field stays where it
 * is while the rows scroll. Outside a view it is drawn where it stands, as it always was; in a
 * view whose top is not mounted yet it is drawn once it is. The field stays in the list's React
 * tree, so it keeps the list's state; in the document it is in the top, before the rows, which is
 * the order the keyboard reaches them in.
 */
export function InPanelTop({ children }: { children: ReactNode }) {
  const slot = useContext(TopSlot);
  if (slot === undefined) return <>{children}</>;
  return slot === null ? null : createPortal(children, slot);
}

/** **A section's heading line**: its title, and the section's actions after it. */
export function PanelSectionHead({
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style">) {
  return (
    <div {...rest} data-scope="panel-section" data-part="head">
      {children}
    </div>
  );
}

/**
 * **A section's title**, in the label's capitals, with its mark before it where it has one. A
 * heading of the level its place in the page says (`level`, 2 by default). `className` places it
 * (a margin, a flex share), never a look.
 */
export function PanelSectionTitle({
  level = 2,
  mark: Mark,
  className,
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"h2">, "style"> & {
  level?: 2 | 3;
  /** The mark before the words, decoration beside them. */
  mark?: ComponentType<{ className?: string; "aria-hidden"?: boolean | "true" }>;
  children?: ReactNode;
}) {
  const Heading = level === 3 ? "h3" : "h2";
  return (
    // The label's text role, the one a section label wears everywhere (#1920); a theme's parts
    // reach it over the role.
    <Heading
      {...rest}
      className={clsx("ui-text-label", className)}
      data-scope="panel-section"
      data-part="title"
    >
      {Mark && <Mark className="node-icon" aria-hidden="true" />}
      {children}
    </Heading>
  );
}
