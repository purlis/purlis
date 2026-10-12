// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useId, useLayoutEffect, useRef } from "react";
import clsx from "clsx";
import { ChevronLeft } from "lucide-react";

/**
 * **A page behind one of the welcome's doors** (#1812): Create a project, Get a project from
 * your forge, or Sign in to GitHub or GitLab.
 *
 * It is drawn in the welcome's place, at the welcome's width, with **Back** first and the page's
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
  /** Comes back to the welcome. */
  onBack: () => void;
  children: ReactNode;
}) {
  const id = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  useLayoutEffect(() => {
    heading.current?.focus();
  }, []);
  return (
    <section className={clsx("welcome", "welcome-page", className)} aria-labelledby={id}>
      <header className="welcome-page-head">
        <button type="button" tabIndex={0} className="welcome-link" onClick={onBack}>
          <ChevronLeft />
          Back
        </button>
        {/* `-1`: reached by the page appearing, never by Tab, which goes on to the acts. */}
        <h1 id={id} tabIndex={-1} ref={heading}>
          {title}
        </h1>
      </header>
      {children}
    </section>
  );
}
