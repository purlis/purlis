// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type NeversToKeep, type PlaneId, type PolicyState } from "./bindings";
import { Notice } from "./Notice";
import { inTheWindow } from "./personaHostsAllow";
import { holdPolicy } from "./projectPolicy";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { sandboxCommandReturned } from "./sandboxAsked";
import { inertly } from "./sandboxBlocks";
import { useWindowFocused } from "./windowFocused";

/** Whether this page may accept the project's policy: the window may, a link may not. */
export const mayAcceptPolicy = (): boolean => inTheWindow(commands, "acceptProjectPolicy");

/** What Accept means, after the families that wait. */
export const POLICY_ACCEPT_MEANS =
  "Accept, and your chats here run that way from now on. Until you do, they keep what you accepted before.";

/** What the window says where this machine's acceptance does not hold: every family enforced. */
export const POLICY_UNSEALED_SAID =
  "The policy this machine kept as accepted does not hold, so every family is enforced and no chat dispatches to another persona until you accept the project's policy again.";

/**
 * What the window says where a new seal key left the project's never-dispatch list unread
 * (#7714). Accept never keeps it: Keep these nevers is a press of its own, and it seals exactly
 * the pairs shown (H1 of #7734's round 3).
 */
export const NEVERS_NEW_KEY_SAID =
  "purlis made a new seal key on this machine, so it cannot vouch for your never-dispatch list, and no chat dispatches to another persona until you keep the pairs it holds now.";

/**
 * What the window says of the pairs Keep these nevers seals: that it holds none, that its file
 * does not read, or that it holds a name purlis cannot show as it is (#7734's round 4), each with
 * nothing offered where nothing can be kept. The pairs themselves are drawn apart
 * ({@link NeverPairs}), never joined into a sentence, so no name can read as several pairs.
 */
export const neversToKeepSaid = (kept: NeversToKeep | null, unshowable: boolean): string => {
  if (kept === null)
    return unshowable
      ? "It holds a name no persona can have, which purlis cannot show as it is, so there is nothing to keep. Fix the file, and its pairs show here."
      : "Its file does not read, so there is nothing to keep yet. Fix the file, and its pairs show here.";
  if (kept.pairs.length === 0)
    return "It holds no pair now. Keep these nevers keeps it empty, so no pair is refused.";
  return `Keep these nevers seals exactly ${kept.pairs.length === 1 ? "the pair" : "the pairs"} below.`;
};

/**
 * What the window says where a list purlis cannot vouch for keeps the project's never-dispatch
 * list unread, and no delete helps (#7852). Seal these nevers is the way out: it seals exactly the
 * pairs shown and clears the project's dispatch grants, so every dispatch asks again.
 */
export const NEVERS_UNVOUCHED_SAID =
  "purlis cannot vouch that your never-dispatch list is whole, and something other than purlis may have changed it, so no chat dispatches to another persona until you seal the pairs it holds now.";

/** What Seal these nevers does beyond sealing the pairs: the grants it clears. */
const SEAL_CLEARS =
  "and clears this project's dispatch grants on this machine, so every dispatch to another persona asks you again until you grant it.";

/**
 * What the window says of the pairs Seal these nevers seals (#7852), as {@link neversToKeepSaid}
 * says those Keep these nevers keeps, with the grants it clears.
 */
export const neversToSealSaid = (kept: NeversToKeep | null, unshowable: boolean): string => {
  if (kept === null)
    return unshowable
      ? "It holds a name no persona can have, which purlis cannot show as it is, so there is nothing to seal. Fix the file, and its pairs show here."
      : "Its file does not read, so there is nothing to seal yet. Fix the file, and its pairs show here.";
  if (kept.pairs.length === 0)
    return `It holds no pair now. Seal these nevers keeps it empty ${SEAL_CLEARS}`;
  return `Seal these nevers seals exactly ${kept.pairs.length === 1 ? "the pair" : "the pairs"} below ${SEAL_CLEARS}`;
};

/** The pairs a nevers press seals, each an item of its own with each name as code. */
export function NeverPairs({
  kept,
  label = "The pairs Keep these nevers seals",
}: {
  kept: NeversToKeep | null;
  label?: string;
}) {
  if (kept === null || kept.pairs.length === 0) return null;
  return (
    <ul className="granted-list" aria-label={label}>
      {kept.pairs.map((one, at) => (
        // The list is drawn once per answer, and two pairs may be alike: the place is the key.
        <li key={at}>
          <code>{one.asking}</code> never dispatches to <code>{one.target}</code>
        </li>
      ))}
    </ul>
  );
}

/** Keep these nevers: the pairs shown, by their digest, on the project `plane` names. */
export const keepNevers = (plane: PlaneId, kept: NeversToKeep) =>
  commands.keepProjectNevers(plane, kept.digest);

/** Seal these nevers: the pairs shown, by their digest, on the project `plane` names (#7852). */
export const sealNevers = (plane: PlaneId, kept: NeversToKeep) =>
  commands.sealUnvouchedProjectNevers(plane, kept.digest);

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
  const [neversSaid, setNeversSaid] = useState<string>();
  const [neversHidden, setNeversHidden] = useState(false);

  useEffect(() => {
    let live = true;
    void commands
      .projectPolicyState(plane)
      .then((answer) => {
        // Only a policy state is drawn: an answer of any other shape says nothing.
        if (live && answer.status === "ok" && drawable(answer.data)) {
          setState(answer.data);
          // The pane heads say it from here: one read per project (`projectPolicy.ts`).
          holdPolicy(plane, answer.data);
        }
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
        if (drawable(done.data)) {
          setState(done.data);
          holdPolicy(plane, done.data);
        }
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

  const pressNevers = (kept: NeversToKeep | null, press: typeof keepNevers) => {
    if (busy || kept === null) return;
    setBusy(true);
    setNeversSaid(undefined);
    void press(plane, kept)
      .then((done) => {
        if (done.status !== "ok") {
          setNeversSaid(done.error);
          setAgain((n) => n + 1);
          return;
        }
        if (drawable(done.data)) {
          setState(done.data);
          holdPolicy(plane, done.data);
        }
        sandboxCommandReturned();
      })
      .catch((err: unknown) =>
        setNeversSaid(`purlis could not keep your never-dispatch list: ${String(err)}`),
      )
      .finally(() => setBusy(false));
  };
  const keep = () => pressNevers(state.nevers_to_keep, keepNevers);
  const seal = () => pressNevers(state.nevers_to_seal, sealNevers);

  const neversBody = (
    <>
      <p>
        {`${NEVERS_NEW_KEY_SAID} ${neversToKeepSaid(state.nevers_to_keep, state.nevers_unshowable)}`}
        {!canAccept && " Keep them from purlis's own window on this machine."}
        {neversSaid !== undefined && ` ${neversSaid}`}
      </p>
      <NeverPairs kept={state.nevers_to_keep} />
    </>
  );
  const unvouchedBody = (
    <>
      <p>
        {`${NEVERS_UNVOUCHED_SAID} ${neversToSealSaid(state.nevers_to_seal, state.nevers_unshowable)}`}
        {!canAccept && " Seal them from purlis's own window on this machine."}
        {neversSaid !== undefined && ` ${neversSaid}`}
      </p>
      <NeverPairs kept={state.nevers_to_seal} label="The pairs Seal these nevers seals" />
    </>
  );
  const nevers = neversHidden ? null : state.nevers_new_key ? (
    canAccept && state.nevers_to_keep !== null ? (
      <Notice
        cause="nevers-new-key"
        label="Your never-dispatch list"
        fixes={[{ label: "Keep these nevers", onPress: keep, busy }]}
      >
        {neversBody}
      </Notice>
    ) : (
      <Notice
        cause="nevers-new-key"
        label="Your never-dispatch list"
        onDismiss={() => setNeversHidden(true)}
      >
        {neversBody}
      </Notice>
    )
  ) : state.nevers_unvouched ? (
    canAccept && state.nevers_to_seal !== null ? (
      <Notice
        cause="nevers-unvouched"
        label="Your never-dispatch list"
        fixes={[{ label: "Seal these nevers", onPress: seal, busy }]}
      >
        {unvouchedBody}
      </Notice>
    ) : (
      <Notice
        cause="nevers-unvouched"
        label="Your never-dispatch list"
        onDismiss={() => setNeversHidden(true)}
      >
        {unvouchedBody}
      </Notice>
    )
  ) : null;

  const waits = state.waiting.length > 0 || state.unsealed;
  if (!waits || hidden)
    return (
      <>
        {refused}
        {nevers}
      </>
    );

  const body = (
    <p>
      {state.unsealed && `${POLICY_UNSEALED_SAID} `}
      {state.waiting.length > 0 &&
        `The project loosened its policy: ${waitingSaid(state)}. ${notInForceSaid(state)}${POLICY_ACCEPT_MEANS}`}
      {!canAccept && " Accept it from purlis's own window on this machine."}
      {said !== undefined && ` ${said}`}
    </p>
  );
  return (
    <>
      {refused}
      {nevers}
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
