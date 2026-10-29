// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The disclosure family** (#3675, spec #2897, added to the closed family list by the operator
 * on 2026-10-28): a button that shows and hides the region under it, on React Aria's
 * `Disclosure`, `DisclosureGroup` and `DisclosurePanel`, wrapped here once. Add source's
 * **More**, a file to read under its risk mark, a deleted skill's last text. A screen imports
 * these and never `react-aria-components`, and never writes `data-scope="disclosure"` itself
 * (`purlis/disclosure-family`).
 *
 * - **Its trigger is a button of the button family**, the quiet ghost, leading with the shared
 *   twist ({@link Twist}), which turns and never swaps for another chevron. It says
 *   `aria-expanded`, and controls the panel (`aria-controls`).
 * - **Its panel follows it in the reading order**, named by the trigger. Opening it moves only
 *   what is under it: the trigger's row keeps its size, and the twist's turn is a transform.
 * - **It starts folded** unless told otherwise (`defaultExpanded`), and the owner may hold its
 *   state (`isExpanded`, `onExpandedChange`).
 * - **A refused field inside it opens it** (`ui/reveal.ts`): a form that refuses a field in a
 *   folded panel opens every fold around it before the keyboard goes there, so the focus lands and
 *   the field's own error line says why, once (#3291). In a dialog's body and on a page alike.
 * - **By its parts** (ADR 0103): the `root`, the `trigger` row (the button and any mark after
 *   it, `trail`), the `twist` and the `panel`, each `data-scope="disclosure"`; `disclosure.css`
 *   draws the family default and a theme's parts reach the same attributes.
 */
import { useEffect, useRef, type ReactNode } from "react";
import { flushSync } from "react-dom";
import {
  Disclosure as AriaDisclosure,
  DisclosureGroup as AriaDisclosureGroup,
  DisclosurePanel as AriaDisclosurePanel,
} from "react-aria-components";
import type { DisclosureState } from "react-stately/useDisclosureState";
import { ChevronRight } from "lucide-react";
import { Button, type ButtonSize } from "../button/Button";
import { REVEAL_EVENT } from "../reveal";

/**
 * **The shared twist** (#3675, #1916): one chevron that points at its words while folded and
 * turns down while open, mirrored in a right-to-left window. The disclosure's trigger leads with
 * it; a fold that is not a disclosure (a tree's group of rows, a table's row, a menu's item) draws
 * it in place of swapping two chevrons. Hidden from a screen reader: what it says is its
 * control's `aria-expanded`.
 */
export function Twist({ open }: { open?: boolean }) {
  return (
    <ChevronRight
      aria-hidden="true"
      focusable="false"
      data-scope="disclosure"
      data-part="twist"
      data-open={open ? "" : undefined}
    />
  );
}

/** The panel's attributes: a group or a region, never a live one. */
const panelOf = (role: "group" | "region") =>
  ({ role, "data-scope": "disclosure", "data-part": "panel" }) as const;

/** Heading levels a trigger's row may stand in, where the fold heads a section of a page. */
type Level = 2 | 3 | 4 | 5 | 6;

export interface DisclosureProps {
  /** The trigger's words: what the panel holds ("More: branch, folder, sign-in"). */
  label: ReactNode;
  /** What the panel holds. */
  children: ReactNode;
  /** Marks after the trigger, in its row: a file's risk mark. Never a control. */
  trail?: ReactNode;
  /** Whether it starts open, when the owner does not hold its state. Folded unless told. */
  defaultExpanded?: boolean;
  /** Whether it is open, for an owner that holds its state. */
  isExpanded?: boolean;
  /** Told each time it opens or folds, by the person or by a refused field in it. */
  onExpandedChange?: (expanded: boolean) => void;
  /** The trigger's row is a heading of this level, where the fold heads a section. */
  level?: Level;
  /** The trigger's size: `sm` in a dense row, `md` (the default) in a page or a dialog. */
  size?: Extract<ButtonSize, "sm" | "md">;
  /** Its id, inside a {@link DisclosureGroup}, to name it in the group's `expandedKeys`. */
  id?: string;
  /** The panel's role: `group` (the default) or `region`, for a large panel a person may jump to. */
  panelRole?: "group" | "region";
  /** A place for it in its view: margin, width. Never a look. */
  className?: string;
  "data-testid"?: string;
}

/** **A disclosure**: the trigger's row, then the panel it shows. */
export function Disclosure({
  label,
  children,
  trail,
  defaultExpanded = false,
  isExpanded,
  onExpandedChange,
  level,
  size = "md",
  id,
  panelRole = "group",
  className,
  "data-testid": testId,
}: DisclosureProps) {
  // React Aria's state, which a group around it holds when there is one: read by the reveal
  // listener, which is set up once.
  const state = useRef<DisclosureState | null>(null);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const at = root.current;
    if (!at) return;
    // A refused field in the panel: open now, so the focus that follows the event lands.
    const open = () => {
      const held = state.current;
      if (!held || held.isExpanded) return;
      flushSync(() => held.expand());
    };
    at.addEventListener(REVEAL_EVENT, open);
    return () => at.removeEventListener(REVEAL_EVENT, open);
  }, []);
  const Row = level === undefined ? "div" : (`h${level}` as const);
  return (
    <AriaDisclosure
      ref={root}
      id={id}
      defaultExpanded={defaultExpanded}
      isExpanded={isExpanded}
      onExpandedChange={onExpandedChange}
      className={className}
      data-testid={testId}
      data-scope="disclosure"
      data-part="root"
    >
      {({ state: now }) => {
        state.current = now;
        return (
          <>
            <Row data-scope="disclosure" data-part="trigger">
              <Button
                slot="trigger"
                variant="quiet"
                size={size}
                // Said here too, so the button family reads it as a disclosure's and never draws
                // it as a trigger whose menu is open (`ui/pressable.tsx`).
                aria-expanded={now.isExpanded}
                icon={<Twist />}
              >
                {label}
              </Button>
              {trail}
            </Row>
            <AriaDisclosurePanel {...panelOf(panelRole)}>{children}</AriaDisclosurePanel>
          </>
        );
      }}
    </AriaDisclosure>
  );
}

/**
 * **Disclosures that open together or one at a time** (an accordion): the review's files, where
 * opening one may fold the others (`allowsMultipleExpanded`, off unless told). The owner may hold
 * which are open (`expandedKeys`, by each disclosure's `id`).
 */
export function DisclosureGroup({
  children,
  allowsMultipleExpanded = false,
  expandedKeys,
  defaultExpandedKeys,
  onExpandedChange,
  className,
}: {
  children: ReactNode;
  allowsMultipleExpanded?: boolean;
  expandedKeys?: Iterable<string>;
  defaultExpandedKeys?: Iterable<string>;
  onExpandedChange?: (keys: Set<string>) => void;
  className?: string;
}) {
  return (
    <AriaDisclosureGroup
      allowsMultipleExpanded={allowsMultipleExpanded}
      expandedKeys={expandedKeys}
      defaultExpandedKeys={defaultExpandedKeys}
      onExpandedChange={(keys) => onExpandedChange?.(new Set([...keys].map(String)))}
      className={className}
      data-scope="disclosure"
      data-part="group"
    >
      {children}
    </AriaDisclosureGroup>
  );
}
