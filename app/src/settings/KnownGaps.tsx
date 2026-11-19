// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type HarnessGaps, type KnownGapShown, type PlaneId } from "../bindings";
import { harnessSaid } from "../chatLog";
import type { RowIds } from "./components";
import type { Setting } from "./groups";

/** The address of the setting that holds the known gaps, in the Sandbox group (M75-37). */
export const KNOWN_GAPS = "project.sandbox.knownGaps";

/** The sentence that says what a declared gap does to a chat: nothing, to its start. */
export const STILL_STARTS =
  "A known gap never stops a chat from starting. A project that wants a start refused writes a rule on start.";

/** One gap, as a line of the list: what it is, what purlis does instead, and the families whose
 *  rules it leaves unenforced. */
export function gapSaid(gap: KnownGapShown): string {
  const families = gap.families.length === 0 ? "" : ` Affects ${gap.families.join(", ")}.`;
  return `${gap.said} ${gap.instead}${families}`;
}

/**
 * **Known gaps, declared per harness** (M75-37, #7046; spec §14.8, Q319 C1): each place a rule in
 * force cannot be enforced the way it is elsewhere, with what purlis does there. Drawn from what
 * the core declares for this project's modes on this system; a rule that is `off` declares none.
 */
export function KnownGapsList({
  harnesses,
  ids,
}: {
  harnesses: readonly HarnessGaps[];
  ids: RowIds;
}) {
  const declared = harnesses.filter((one) => one.gaps.length > 0);
  return (
    <div id={ids.id} aria-labelledby={ids.labelledBy}>
      {declared.length === 0 ? (
        <p>No known gaps are declared: every rule in force is enforced here.</p>
      ) : (
        <>
          <p>{STILL_STARTS}</p>
          {declared.map((one) => (
            <section key={one.harness} aria-label={harnessSaid(one.harness)}>
              <h4>{harnessSaid(one.harness)}</h4>
              <ul className="granted-list" aria-label={`${harnessSaid(one.harness)} known gaps`}>
                {one.gaps.map((gap) => (
                  <li key={gap.id} data-gap={gap.id}>
                    <span>{gapSaid(gap)}</span>
                    <span className="granted-note"> {gap.from}</span>
                    {gap.conditional && (
                      <span className="granted-note"> Only for some chats of this harness.</span>
                    )}
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </>
      )}
    </div>
  );
}

/** The Sandbox group's Known gaps row: read from the core when it opens. */
export function KnownGaps({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const [harnesses, setHarnesses] = useState<readonly HarnessGaps[]>();
  const [said, setSaid] = useState<string>();
  useEffect(() => {
    let gone = false;
    void commands
      .knownGaps(plane)
      .then((done) => {
        if (gone) return;
        if (done.status === "error") setSaid(done.error);
        else setHarnesses(done.data ?? []);
      })
      .catch((err: unknown) => {
        if (!gone) setSaid(`purlis could not list the known gaps: ${String(err)}`);
      });
    return () => {
      gone = true;
    };
  }, [plane]);
  if (harnesses === undefined) return <p id={ids.id}>{said ?? "Reading the known gaps…"}</p>;
  return <KnownGapsList harnesses={harnesses} ids={ids} />;
}

/** The Sandbox group's row for them. Nothing here writes a file: the gaps are what the core
 *  declares, and a rule that is switched off declares none. */
export function knownGapsSetting(plane: PlaneId): Setting {
  return {
    id: KNOWN_GAPS,
    label: "Known gaps",
    writes: "nothing",
    help: "Where a rule in force can't be enforced the way it is elsewhere, per harness on this system, and what purlis does there. A rule that is off declares none.",
    useControl: function useKnownGaps() {
      return { control: (ids) => <KnownGaps plane={plane} ids={ids} /> };
    },
  };
}
