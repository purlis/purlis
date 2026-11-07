// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { commands, type DoctorReport, type InboxBaseline, type PlaneId } from "./bindings";
import { findingCauses } from "./Doctor";

/**
 * **The first-open baseline** (#5648, ADR 0117): the first time a project opens on a machine
 * with no record of it (`Store::opened_before`), what is already there counts as seen, so a fresh clone opens
 * quiet. The core keeps the record and, at that open, writes the teammate Notices' seen records
 * (`inbox_first_open`); a project with a record is never baselined again.
 *
 * **What the window does with it**: the doctor runs after the project opens, so its first
 * report after a fresh open is the one whose findings were already there. The core keeps them,
 * once, and each is dismissed from the doctor's Notice line, which lasts until the finding
 * changes (`dismissals.ts`): the line and the Notices dot stay quiet, and a finding that
 * appears later raises the line again.
 *
 * `ready` is false until the core has answered, so the teammate Notices read the project only
 * after their seen records are written. A core that cannot keep the record answers no
 * baseline, and nothing is set aside: the safe way to be wrong is to tell too much.
 */
export function useFirstOpen({
  plane,
  report,
  setAside,
  dismiss,
}: {
  plane: PlaneId;
  /** The doctor's last report. */
  report: DoctorReport | undefined;
  /** Keeps the findings already there, by cause, in the baseline (`set_aside_inbox_findings`). */
  setAside: (keys: readonly string[]) => void;
  /** Dismisses a Notice's cause until it changes. */
  dismiss: (cause: string) => void;
}): { ready: boolean } {
  const [held, setHeld] = useState<{ plane: PlaneId; baseline: InboxBaseline | null }>();
  const baseline = held?.plane === plane ? held.baseline : undefined;

  useEffect(() => {
    let live = true;
    const took = (baseline: InboxBaseline | null) => {
      if (live) setHeld({ plane, baseline });
    };
    void commands
      .inboxFirstOpen(plane)
      .then((said) => took(said.status === "ok" ? (said.data ?? null) : null))
      .catch(() => took(null));
    return () => {
      live = false;
    };
  }, [plane]);

  /** The project whose first report's findings this window set aside: once per project. */
  const tookFor = useRef<PlaneId>(undefined);
  const taking = baseline?.fresh === true && baseline.findings === null && report !== undefined;
  useEffect(() => {
    if (!taking || report === undefined || tookFor.current === plane) return;
    tookFor.current = plane;
    const causes = findingCauses(report);
    setAside(causes);
    for (const cause of causes) dismiss(cause);
  }, [taking, report, plane, setAside, dismiss]);

  return { ready: baseline !== undefined };
}
