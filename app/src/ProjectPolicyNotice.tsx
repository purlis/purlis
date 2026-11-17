// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type PolicyState } from "./bindings";
import { Notice } from "./Notice";
import { inTheWindow } from "./personaHostsAllow";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { sandboxCommandReturned } from "./sandboxAsked";
import { inertly } from "./sandboxBlocks";
import { useWindowFocused } from "./windowFocused";

/** Whether this page may accept the project's policy: the window may, a link may not. */
export const mayAcceptPolicy = (): boolean => inTheWindow(commands, "acceptProjectPolicy");

/** What Accept means, after the families that wait. */
export const POLICY_ACCEPT_MEANS =
  "Accept, and your chats here run that way from now on. Until you do, they keep what you accepted before.";

/** Whether an answer is a policy state to draw: one of any other shape says nothing. */
const drawable = (data: PolicyState | null | undefined): data is PolicyState =>
  Array.isArray(data?.waiting);

/** The families that wait, one clause each: "guards.merge from enforce to log". */
export const waitingSaid = (state: PolicyState): string =>
  state.waiting.map((one) => `${one.family} from ${one.accepted} to ${one.committed}`).join(", ");

/**
 * What runs without a sandbox leaves out (Q308a): "Without a sandbox, network, ports and
 * processes are not in force. ", or nothing where the project keeps one.
 */
export const notInForceSaid = (state: PolicyState): string => {
  const families = state.not_in_force ?? [];
  if (families.length === 0) return "";
  const listed =
    families.length === 1
      ? families[0]
      : `${families.slice(0, -1).join(", ")} and ${families[families.length - 1]}`;
  return `Without a sandbox, ${listed} are not in force. `;
};

/**
 * **A loosening of the project's `[policy]` waits for your press** (rulings Q308a, Q308b; ADR
 * 0125): a committed change that makes a family looser reaches your chats only once you press
 * **Accept** here (`accept_project_policy`, the window's alone), which the app seals on this
 * machine. A tightening needs no press. Beside it, **what the file says that purlis could not
 * read** is said, each value enforced meanwhile, and an acceptance whose seal does not hold.
 */
export function ProjectPolicyNotice({
  plane,
  canAccept = mayAcceptPolicy(),
}: {
  plane: PlaneId;
  /** Whether this page may accept: the window may, a link may not. */
  canAccept?: boolean;
}) {
  const [state, setState] = useState<PolicyState>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const onDisk = usePlaneChanged([plane], SETTINGS);
  const focused = useWindowFocused();
  const [again, setAgain] = useState(0);
  const [hidden, setHidden] = useState(false);
  const [refusedHidden, setRefusedHidden] = useState(false);

  useEffect(() => {
    let live = true;
    void commands
      .projectPolicyState(plane)
      .then((answer) => {
        // Only a policy state is drawn: an answer of any other shape says nothing.
        if (live && answer.status === "ok" && drawable(answer.data)) setState(answer.data);
      })
      // A state that cannot be read asks nothing: until an Accept, nothing is loosened.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused, again]);

  if (state === undefined) return null;

  const accept = () => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .acceptProjectPolicy(plane, state.digest)
      .then((done) => {
        if (done.status !== "ok") {
          setSaid(done.error);
          setAgain((n) => n + 1);
          return;
        }
        if (drawable(done.data)) setState(done.data);
        sandboxCommandReturned();
      })
      .catch((err: unknown) => setSaid(`purlis could not keep your acceptance: ${String(err)}`))
      .finally(() => setBusy(false));
  };

  const refused =
    state.refused.length > 0 && !refusedHidden ? (
      <Notice
        cause="policy-unreadable"
        label="The project's policy"
        onDismiss={() => setRefusedHidden(true)}
      >
        <p>
          {state.refused.map((one) => inertly(one)).join(". ")}. Fix the project's settings file;
          until then every family is enforced.
        </p>
      </Notice>
    ) : null;

  const waits = state.waiting.length > 0 || state.unsealed;
  if (!waits || hidden) return refused;

  const body = (
    <p>
      {state.unsealed &&
        "The policy this machine kept as accepted does not hold, so every family is enforced and no chat dispatches to another persona until you accept the project's policy again. "}
      {state.waiting.length > 0 &&
        `The project loosened its policy: ${waitingSaid(state)}. ${notInForceSaid(state)}${POLICY_ACCEPT_MEANS}`}
      {!canAccept && " Accept it from purlis's own window on this machine."}
      {said !== undefined && ` ${said}`}
    </p>
  );
  return (
    <>
      {refused}
      {canAccept && (state.waiting.length > 0 || state.unsealed) ? (
        <Notice
          cause="policy-loosened"
          label="The project's policy"
          fixes={[{ label: "Accept", onPress: accept, busy }]}
        >
          {body}
        </Notice>
      ) : (
        <Notice
          cause="policy-loosened"
          label="The project's policy"
          onDismiss={() => setHidden(true)}
        >
          {body}
        </Notice>
      )}
    </>
  );
}
