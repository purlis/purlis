// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { Lock } from "lucide-react";

import { commands, type PlaneId, type ReadOnlyProject } from "./bindings";
import { SETTINGS, usePlaneChanged } from "./planeChanged";

/**
 * Why this purlis opens `plane` read-only, or null when it may write it (#826, FR-24).
 *
 * Asked at the mount and again whenever the project's own files change (`SETTINGS` is what a
 * change to its manifest concerns), since a pull that adds a `requires` entry makes the project
 * read-only from that moment. A window that cannot ask says nothing: the core refuses every
 * write of a committed file whatever the window shows, so silence never lets one through.
 *
 * Every action that writes a committed project file is disabled with `sentence` as its reason.
 */
export function useReadOnly(plane: PlaneId): ReadOnlyProject | null {
  const changes = usePlaneChanged([plane], SETTINGS);
  // Kept with the project it is about, so another project's tab never shows it for a moment.
  const [read, setRead] = useState<{ plane: PlaneId; readOnly: ReadOnlyProject | null }>();
  useEffect(() => {
    let gone = false;
    void commands
      .projectReadOnly(plane)
      .then((answer) => {
        if (!gone && answer.status === "ok") setRead({ plane, readOnly: answer.data });
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, changes]);
  return read?.plane === plane ? read.readOnly : null;
}

/**
 * The project tab's one sentence when this purlis opens it read-only: which feature is missing
 * and which version has it. Said once, in the tab; the disabled actions repeat it as their
 * reason. Nothing is drawn for a project this purlis may write.
 */
export function ReadOnlyLine({ readOnly }: { readOnly: ReadOnlyProject | null }) {
  if (!readOnly) return null;
  return (
    <p className="read-only-line" role="note" aria-label="Read-only project">
      <Lock aria-hidden="true" />
      <span>{readOnly.sentence}</span>
    </p>
  );
}

/**
 * **Why the project this is drawn in is read-only, as its window lends it** (#826, FR-24): the
 * core's sentence, or undefined where purlis may write the project or outside a project's
 * window. A surface that writes a committed project file without going through the catalogue
 * (a memory's Save, a persona's mark, Settings' project and workspace levels, an extension's
 * row action) reads it here and turns that control off with the sentence as its reason. The
 * line itself is said once, on the project's tab ({@link ReadOnlyLine}); a control repeats it
 * only as its tooltip.
 */
export const ReadOnlySaid = createContext<string | undefined>(undefined);

/** The sentence a read-only project's write controls are off with, or undefined. */
export function useReadOnlySaid(): string | undefined {
  return useContext(ReadOnlySaid);
}

/**
 * Whether Edit as TOML may still replace the project's settings file whole on a read-only
 * project: only where that file is the manifest this purlis cannot read as TOML, which that
 * editor is how the person mends (#1759). The core lets that one save through and no other.
 */
export const ReadOnlyMendable = createContext<boolean>(false);

/** {@link ReadOnlyMendable}, read. */
export function useReadOnlyMendable(): boolean {
  return useContext(ReadOnlyMendable);
}

/**
 * **A group of controls that write a committed project file, off on a read-only project**: a
 * native `<fieldset>`, `disabled` with the sentence as its tooltip, so every box, pick and
 * button in it is off at once and says why on hover. Always drawn, so the controls under it are
 * never remounted when the answer arrives; it draws no box of its own (`.read-only-off`).
 */
export function OffWhenReadOnly({ children }: { children: ReactNode }) {
  const said = useReadOnlySaid();
  return (
    <fieldset className="read-only-off" disabled={said !== undefined} title={said}>
      {children}
    </fieldset>
  );
}
