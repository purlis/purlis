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
 */
import clsx from "clsx";
import type { ComponentPropsWithRef, ComponentType, ReactNode } from "react";

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

/** **A section of a panel**: its heading line, then what it holds. */
export function PanelSection({
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"section">, "style"> & {
  [data: `data-${string}`]: string | undefined;
}) {
  return (
    <section {...rest} data-scope="panel-section" data-part="root" data-variant="default">
      {children}
    </section>
  );
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
