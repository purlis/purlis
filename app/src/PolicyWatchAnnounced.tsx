// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId } from "./bindings";
import { seenElsewhereOnThisMachine, useSeenOnThisMachine } from "./dismissals";
import { Notice } from "./Notice";
import { useHeldPolicy } from "./projectPolicy";
import { askSettingsLink } from "./settings/links";
import { POLICY } from "./settings/policy";

/** The Notice's cause: what the person has seen, kept on this machine (`dismissals.ts`). */
export const POLICY_WATCH_ANNOUNCED = "policy-watch-announced";

/** What it says: blocks are off by default now, and how to turn one back on. */
export const POLICY_WATCH_SAID =
  "Blocks are now off by default. A project that names no policy runs the watch template: " +
  "most of purlis's refusals are logged, not enforced, and secrets, vaults and the credential " +
  "scan stay enforced. To turn a family back on, set it under [policy] in the project's " +
  'settings file, such as guards.merge = "enforce", or template = "strict" for every family, ' +
  "and commit it.";

/**
 * **The upgrade to `watch` is announced once** (ruling Q308a, "Upgrade"; instruction 45): the
 * first project whose policy the window reads and that names no template, so runs the shipped
 * default, says that blocks are now off by default and how to turn one back on in `[policy]`.
 * Dismiss and Open policy settings both keep it as seen **on this machine**, in the person's
 * layout file (`seen_on_this_machine`, as the chip's Notice is), never in a project, which would
 * carry it to every clone. The file is asked again before it is drawn, so a second window open at
 * once does not show it a second time. No chat can set or clear it.
 */
export function PolicyWatchAnnounced({ plane }: { plane: PlaneId }) {
  const policy = useHeldPolicy(plane);
  const { seen, see } = useSeenOnThisMachine(POLICY_WATCH_ANNOUNCED);
  const owed = policy?.template_default === true && !seen;
  const unseen = useUnseen(owed);
  if (!owed || unseen !== true) return null;
  return (
    <Notice
      cause={POLICY_WATCH_ANNOUNCED}
      label="Blocks are off by default"
      fixes={[
        {
          label: "Open policy settings",
          onPress: () => {
            see();
            askSettingsLink(plane, { group: POLICY });
          },
        },
      ]}
      onDismiss={see}
    >
      <p>{POLICY_WATCH_SAID}</p>
    </Notice>
  );
}

/**
 * Whether the layout file still says it was not seen on this machine, asked once it is owed:
 * another window may have seen it since this one launched (then it is held as seen here too).
 * `undefined` until the file answers; anything but a plain no is seen.
 */
function useUnseen(owed: boolean): boolean | undefined {
  const [unseen, setUnseen] = useState<boolean>();
  useEffect(() => {
    if (!owed) return;
    let current = true;
    void commands
      .seenOnThisMachine(POLICY_WATCH_ANNOUNCED)
      .then((answer) => {
        const no = answer.status === "ok" && answer.data === false;
        if (answer.status === "ok" && answer.data === true)
          seenElsewhereOnThisMachine(POLICY_WATCH_ANNOUNCED);
        if (current) setUnseen(no);
      })
      .catch(() => current && setUnseen(false));
    return () => {
      current = false;
    };
  }, [owed]);
  return unseen;
}
