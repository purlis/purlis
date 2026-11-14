// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { CloneToVouch } from "./bindings";
import { FixWay } from "./Notice";
import { ButtonRow } from "./ui/button/Button";
import { Checkbox, CheckboxGroup } from "./ui/choice/Checkbox";

/**
 * **The clones of Trust these clones, on its row in Asks** (T1-42, R-337, Q184): each clone
 * purlis has no record of, with its remote and when it was made, and the boxes Choose… opens
 * under the row. Every folder and address is the core's, read from the clone and drawn as text.
 */

/** The option id of Trust these clones' Choose…, as the core sends it (`asking::CHOOSE`). */
export const CHOOSE = "choose";

/** What the list of clones is called, to the screen reader. */
export const CLONES_LISTED = "Clones purlis did not clone";

/** What Choose…'s boxes are called. */
export const CHOOSE_CLONES = "Choose the clones to trust";

/** When a clone was made, as the row says it: a day, or that its disk keeps no such time. */
function Made({ at }: { at: number | null }) {
  if (at === null) return <>made at a time its disk doesn't keep</>;
  const when = new Date(at * 1000);
  return (
    <>
      made{" "}
      <time dateTime={when.toISOString()}>
        {when.toLocaleDateString([], { year: "numeric", month: "short", day: "numeric" })}
      </time>
    </>
  );
}

/** The address a clone's remote names, as the person reads it: one, or each joined by "and". */
const remoteOf = (clone: CloneToVouch) => clone.place ?? clone.places.join(" and ");

/** **Each clone, with its remote and when it was made** (R-337). */
export function ClonesToTrust({ clones }: { clones: readonly CloneToVouch[] }) {
  return (
    <ul className="inbox-clones ui-text-secondary" aria-label={CLONES_LISTED}>
      {clones.map((clone) => (
        <li key={clone.folder}>
          {clone.folder} · {remoteOf(clone)} · <Made at={clone.made ?? null} />
        </li>
      ))}
    </ul>
  );
}

/**
 * **Choose…'s boxes** (R-337): one for each clone, none ticked, and Trust chosen, which trusts
 * only the ticked ones and waits for the row to settle (`mayAct`, ADR 0097) as every trust does.
 * Cancel puts the boxes away. A clone whose remote names several addresses can't be trusted, so
 * its box is off and says why.
 */
export function TrustClonesChooser({
  clones,
  busy,
  mayAct,
  onTrust,
  onCancel,
}: {
  clones: readonly CloneToVouch[];
  busy: boolean;
  mayAct: (way: Element) => boolean;
  onTrust: (chosen: readonly CloneToVouch[]) => void;
  onCancel: () => void;
}) {
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  const tick = (folder: string, on: boolean) =>
    setChosen((was) => {
      const next = new Set(was);
      if (on) next.add(folder);
      else next.delete(folder);
      return next;
    });
  return (
    <>
      <CheckboxGroup aria-label={CHOOSE_CLONES} className="inbox-clones-chosen">
        {clones.map((clone) => (
          <Checkbox
            key={clone.folder}
            isSelected={chosen.has(clone.folder)}
            isDisabled={clone.place === null}
            onChange={(on) => tick(clone.folder, on)}
            description={
              clone.place === null
                ? `Its remote names ${remoteOf(clone)}. Give origin one address, then trust it.`
                : remoteOf(clone)
            }
          >
            {clone.folder}
          </Checkbox>
        ))}
      </CheckboxGroup>
      <ButtonRow size="sm" role="group" aria-label="Trust the clones chosen">
        <FixWay
          fix={{
            label: "Trust chosen",
            allows: true,
            busy,
            off: chosen.size === 0 ? "Tick the clones to trust first." : undefined,
            onPress: () => onTrust(clones.filter((clone) => chosen.has(clone.folder))),
          }}
          mayAct={mayAct}
        />
        <FixWay fix={{ label: "Cancel", onPress: onCancel }} mayAct={() => true} />
      </ButtonRow>
    </>
  );
}
