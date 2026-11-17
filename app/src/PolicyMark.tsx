// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Scale } from "lucide-react";
import type { PlaneId, PolicyState } from "./bindings";
import { policySentence, useHeldPolicy } from "./projectPolicy";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **The project's policy on a chat's pane head** (rulings Q308a, Q308b; instruction 45): the
 * template's word, and `+N` for the families the project sets apart from it, with the whole
 * sentence on its title and to a screen reader, as the sandbox's mark says its level. Nothing
 * until the policy has been read. It only says: Settings › Project › Policy is where it is read
 * whole, and a loosening's Accept is the project's Notice.
 */
export function PolicyMark({ plane }: { plane: PlaneId }) {
  const state = useHeldPolicy(plane);
  if (state === undefined) return null;
  return <PolicyMarkOf state={state} />;
}

/** {@link PolicyMark}, for a policy in hand: what its stories draw. */
export function PolicyMarkOf({ state }: { state: PolicyState }) {
  const said = policySentence(state);
  const set = state.overrides.length;
  return (
    <Tooltip tip={said}>
      <span
        className="pane-policy ui-text-meta"
        data-policy={state.template}
        role="img"
        aria-label={said}
      >
        <Scale className="tab-mark" aria-hidden="true" />
        <span aria-hidden="true">
          {state.template}
          {set > 0 && ` +${set}`}
        </span>
      </span>
    </Tooltip>
  );
}
