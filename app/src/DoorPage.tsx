// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useId, useLayoutEffect, useRef } from "react";
import clsx from "clsx";
import { ChevronLeft } from "lucide-react";
import { Button } from "./ui/button/Button";

/**
 * **A page behind one of the welcome's doors** (#1812): Create a project, Get a project from
 * your forge, or Sign in to GitHub or GitLab.
 *
 * It is drawn in the launcher's right pane ({@link Launcher}), with **Back** first and the page's
 * heading after it. The keyboard is put on the heading as the page appears, before paint, so a
 * screen reader says where the person has arrived and Tab goes on from the top of the page;
 * Back hands it to the door that was pressed (`Welcome.tsx`). Back changes nothing, so it is
 * Back and not Cancel (`docs/ui-copy.md`): nothing on these pages has been done until its own
 * act is pressed.
 */
export function DoorPage({
  title,
  className,
  onBack,
  children,
}: {
  /** The page's heading: the door's own title, so the person sees they arrived where they
   *  pressed. */
  title: string;
  /** The page's own class beside `welcome-page`, for what only it draws. */
  className?: string;
  /** Comes back to the welcome. A step with nothing behind it to go back to (the guided
   *  steps' harness step, #1817: the project is made) draws no Back. */
  onBack?: () => void;
  children: ReactNode;
}) {
  const id = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  useLayoutEffect(() => {
    heading.current?.focus();
  }, []);
  return (
    <section className={clsx("welcome-page", className)} aria-labelledby={id}>
      <header className="welcome-page-head">
        {onBack && (
          <Button variant="link" onPress={onBack} icon={<ChevronLeft />} className="welcome-link">
            Back
          </Button>
        )}
        {/* `-1`: reached by the page appearing, never by Tab, which goes on to the acts. */}
        <h1 id={id} tabIndex={-1} ref={heading}>
          {title}
        </h1>
      </header>
      {children}
    </section>
  );
}

/**
 * **The window with no project in front, as a launcher**: the ways in down its left side, always
 * in sight, and the page they open beside them. The left side stays put while the page scrolls,
 * and the whole of it scrolls as one (`.panes`, #3325).
 */
export function Launcher({ side, children }: { side?: ReactNode; children: ReactNode }) {
  return (
    <section className="welcome" aria-label="purlis">
      <div className="welcome-side">
        <div className="welcome-side-in">
          <p className="welcome-brand">purlis</p>
          {side}
        </div>
      </div>
      <div className="welcome-pane">{children}</div>
    </section>
  );
}
