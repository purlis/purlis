// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { RefObject } from "react";
import type { RunShown, RunValue } from "./bindings";
import { TextField } from "./ui/field/Field";

/** Where a run's notice keeps its value boxes, by key: each holds its own words. */
export type RunBoxes = RefObject<Map<string, HTMLInputElement>>;

/**
 * **The value fields of a run's one notice** (#5242, ADR 0112): a masked box for each key the run
 * lacks, beside its program's Allow or, where it has none, the vault's own. Each box holds its
 * own words, so a value is never in React's state or the page's markup; the answer reads them
 * ({@link valuesOf}). What a field stores is said once: always, on a vault the persona uses;
 * only on an Allow, on one it can't. A plain body for now: the ask's own part family replaces it
 * (#5080).
 */
export function RunFills({ run, boxes }: { run: RunShown; boxes: RunBoxes }) {
  if (run.fills.length === 0 && run.no_allow == null) return null;
  return (
    <div data-testid="run-fills">
      {run.no_allow != null && (
        // The app's own words: policy offers no Allow on this run, and storing still works.
        <p className="ui-text-meta" data-testid="run-fills-no-allow">
          {run.no_allow}
        </p>
      )}
      {run.fills.length > 0 && (
        <p className="ui-text-meta">
          {stored(run)} The chat hears what you answered, never a value.
        </p>
      )}
      {run.fills.map((fill) => (
        <div key={fill.key}>
          <p className="ui-text-meta">
            <code>{fill.key}</code>
            {fill.declared != null && <> · {run.persona} declares it for: {fill.declared}</>}
            {fill.gone != null && (
              <>
                {" "}
                · was linked to <code>{fill.gone.key}</code> in vault{" "}
                <code>{fill.gone.vault}</code>, which is gone
              </>
            )}
          </p>
          <TextField
            inputRef={(box: HTMLInputElement | null) => {
              if (box === null) boxes.current?.delete(fill.key);
              else boxes.current?.set(fill.key, box);
            }}
            type="password"
            aria-label={`Value of ${fill.key}`}
            placeholder="Type or paste the value"
          />
        </div>
      ))}
    </div>
  );
}

/** What a field stores, and when, as the notice's answers decide it. */
function stored(run: RunShown): string {
  const lacks = `Vault ${run.vault} lacks ${run.fills.length === 1 ? "this key" : "these keys"}.`;
  if (run.offer === "vault")
    return `${lacks} ${run.persona} can't use the vault otherwise: Allow lets every chat opened as ${run.persona} use it in this project on this machine, and stores what you type. Keep blocked stores nothing.`;
  if (run.offer === "fill") return `${lacks} Fill stores what you type there on this machine.`;
  if (run.not_its_vault)
    return `${lacks} ${run.persona} can't use it otherwise, so what you type is stored there only if you allow the program.`;
  return `${lacks} What you type is stored there whether or not you allow the program.`;
}

/** **Each box's words, by key, read once and emptied**: what the answer sends. */
export function valuesOf(boxes: RunBoxes): RunValue[] {
  const values: RunValue[] = [];
  for (const [key, box] of boxes.current ?? new Map<string, HTMLInputElement>()) {
    values.push({ key, value: box.value });
    box.value = "";
  }
  return values;
}
