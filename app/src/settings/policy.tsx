// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type PolicyLog, type PolicyState } from "../bindings";
import { chatNumberSaid } from "../chatNumber";
import { SETTINGS, usePlaneChanged } from "../planeChanged";
import { holdPolicy, modeSaid, templateSaid } from "../projectPolicy";
import {
  mayAcceptPolicy,
  NEVERS_NEW_KEY_SAID,
  POLICY_ACCEPT_MEANS,
  POLICY_UNSEALED_SAID,
  waitingSaid,
} from "../ProjectPolicyNotice";
import { sandboxCommandReturned } from "../sandboxAsked";
import { inertly } from "../sandboxBlocks";
import { Button, ButtonRow } from "../ui/button/Button";
import { useWindowFocused } from "../windowFocused";
import type { RowIds } from "./components";
import type { SettingsGroup } from "./groups";
import { linkToGroup, settingsPlace } from "./links";

/** The Policy group's address in Settings › Project. */
export const POLICY = "project.policy";

/** The enforcement log's page, under Policy. */
export const POLICY_LOG = "project.policy.log";

/** Whether an answer is a policy state to draw: one of any other shape says nothing. */
const drawable = (data: PolicyState | null | undefined): data is PolicyState =>
  Array.isArray(data?.overrides) && Array.isArray(data?.waiting);

/** The project's policy as the core reads it now, read again when the project's settings change
 *  and after a press the core refused, so a second press names what is shown. */
function usePolicyState(plane: PlaneId) {
  const [state, setState] = useState<PolicyState>();
  const [said, setSaid] = useState<string>();
  const [again, setAgain] = useState(0);
  const onDisk = usePlaneChanged([plane], SETTINGS);
  useEffect(() => {
    let live = true;
    void commands
      .projectPolicyState(plane)
      .then((answer) => {
        if (!live) return;
        if (answer.status !== "ok") setSaid(answer.error);
        else if (drawable(answer.data)) {
          setState(answer.data);
          holdPolicy(plane, answer.data);
        }
      })
      .catch((err: unknown) => live && setSaid(`purlis could not read the policy: ${String(err)}`));
    return () => {
      live = false;
    };
  }, [plane, onDisk, again]);
  const readAgain = () => setAgain((n) => n + 1);
  return { state, setState, said, setSaid, readAgain };
}

/**
 * **The project's policy, as it stands for you** (rulings Q308a, Q308b; instruction 45): its
 * template, each family it sets apart and how your chats run it, what an administrator locks,
 * and a loosening that waits, with Accept where this page is the window. It only says the
 * policy: the core resolves it, and Accept is the same sealed press as the Notice's.
 */
export function PolicyStanding({
  plane,
  ids,
  canAccept = mayAcceptPolicy(),
}: {
  plane: PlaneId;
  ids: RowIds;
  canAccept?: boolean;
}) {
  const { state, setState, said, setSaid, readAgain } = usePolicyState(plane);
  const [busy, setBusy] = useState(false);
  const accept = () => {
    if (state === undefined || busy) return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .acceptProjectPolicy(plane, state.digest)
      .then((done) => {
        if (done.status !== "ok") {
          // Refused, as where the file changed since it was shown: read what stands now.
          setSaid(done.error);
          readAgain();
        } else if (drawable(done.data)) {
          setState(done.data);
          holdPolicy(plane, done.data);
          sandboxCommandReturned();
        }
      })
      .catch((err: unknown) => setSaid(`purlis could not keep your acceptance: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {state === undefined ? (
        said === undefined && <p>Reading the project's policy…</p>
      ) : (
        <>
          <p>
            Template: <code>{templateSaid(state)}</code>.
          </p>
          {state.overrides.length === 0 ? (
            <p>No family is set apart from the template.</p>
          ) : (
            <ul className="granted-list" aria-label="Set apart from the template">
              {state.overrides.map((one) => (
                <li key={one.family}>
                  <code>{one.family}</code> <span>{modeSaid(one)}</span>
                </li>
              ))}
            </ul>
          )}
          {state.locked.length > 0 && (
            <p>This machine's administrator keeps these enforced: {state.locked.join(", ")}.</p>
          )}
          {state.refused.length > 0 && (
            <p>
              {state.refused.map((one) => inertly(one)).join(". ")}. Until the file is fixed, every
              family is enforced.
            </p>
          )}
          {(state.waiting.length > 0 || state.unsealed || state.nevers_new_key) && (
            <>
              <p>
                {state.unsealed && `${POLICY_UNSEALED_SAID} `}
                {state.nevers_new_key && `${NEVERS_NEW_KEY_SAID} `}
                {state.waiting.length > 0 &&
                  `The project loosened its policy: ${waitingSaid(state)}. ${POLICY_ACCEPT_MEANS}`}
                {!canAccept && " Accept it from purlis's own window on this machine."}
              </p>
              {canAccept && (
                <ButtonRow>
                  {/* Accepting loosens what your chats run under: an answer that grants. */}
                  <Button variant="grants" isDisabled={busy} onPress={accept}>
                    Accept
                  </Button>
                </ButtonRow>
              )}
            </>
          )}
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}

/** The project's enforcement log, read again when the project's settings change, when the
 *  window is focused and when its project comes back to the front: a chat may log at any time. */
function usePolicyLog(plane: PlaneId) {
  const [log, setLog] = useState<PolicyLog>();
  const [said, setSaid] = useState<string>();
  const onDisk = usePlaneChanged([plane], SETTINGS);
  // Counts each focus of the window, and each return of its project to the front.
  const focused = useWindowFocused();
  useEffect(() => {
    let live = true;
    void commands
      .policyLog(plane)
      .then((answer) => {
        if (!live) return;
        if (answer.status !== "ok") setSaid(answer.error);
        else if (Array.isArray(answer.data?.lines)) setLog(answer.data);
      })
      .catch(
        (err: unknown) =>
          live && setSaid(`purlis could not read the enforcement log: ${String(err)}`),
      );
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused]);
  return { log, said };
}

/** "Would have blocked: 3", the count Settings shows and the log's page heads. */
export const wouldHaveBlocked = (count: number): string => `Would have blocked: ${count}`;

/**
 * **What the policy let through** (Q308a's count): how many refusals a family set to `log` let
 * through and wrote down, every chat's, with the way to the log that lists them.
 */
export function WouldHaveBlocked({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const { log, said } = usePolicyLog(plane);
  return (
    <div id={ids.id} className="ui-setting-status" aria-labelledby={ids.labelledBy}>
      {log === undefined ? (
        said === undefined && <p>Reading the enforcement log…</p>
      ) : (
        <>
          <p>{wouldHaveBlocked(log.count)}</p>
          {log.count > 0 && (
            <ButtonRow>
              <Button onPress={() => linkToGroup(settingsPlace("project", plane), POLICY_LOG)}>
                Open the enforcement log
              </Button>
            </ButtonRow>
          )}
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}

/** How many lines of the log were left out for not being as purlis writes them. */
const droppedSaid = (dropped: number): string =>
  dropped === 1
    ? "One line of the log is not as purlis writes it, so it is left out."
    : `${dropped} lines of the log are not as purlis writes them, so they are left out.`;

/** One line of the log as the window says it: the family, the rule, the chat and the host. */
function lineSaid(line: PolicyLog["lines"][number]): string {
  const chat = line.session === null ? "a chat purlis does not know" : chatNumberSaid(line.session);
  const host = line.host === null ? "" : `, about ${line.host}`;
  return `${line.family} would have blocked it in ${chat}${host}`;
}

/** **The enforcement log** (Q308a): each block a family set to `log` let through, newest first. */
export function PolicyLogList({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const { log, said } = usePolicyLog(plane);
  return (
    <div id={ids.id} aria-labelledby={ids.labelledBy}>
      {log === undefined ? (
        said === undefined && <p>Reading the enforcement log…</p>
      ) : log.lines.length === 0 ? (
        <p>Nothing would have been blocked here yet.</p>
      ) : (
        <>
          <ul className="granted-list" aria-label="Enforcement log">
            {log.lines.map((line, at) => (
              <li key={`${at}\u0000${line.at}\u0000${line.rule}`}>
                <code>{line.rule}</code> <span>{lineSaid(line)}</span>{" "}
                <span className="granted-note">{line.at}</span>
              </li>
            ))}
          </ul>
          {log.count > log.lines.length && (
            <p>
              The newest {log.lines.length} of {log.count} are shown.
            </p>
          )}
          {log.dropped > 0 && <p>{droppedSaid(log.dropped)}</p>}
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}

/**
 * **Settings › Project › Policy** (instruction 45): the project's `[policy]`, which families of
 * purlis's refusals run, and the count of what the families set to `log` let through.
 */
export function policyGroup(plane: PlaneId, file = "the project's settings file"): SettingsGroup {
  return {
    id: POLICY,
    label: "Policy",
    help: "Which of purlis's blocks run in this project: off, logged or enforced, by a template and the families set apart from it. A loosening reaches your chats once you accept it.",
    also: ["enforcement", "template", "would have blocked", "watch", "strict"],
    kept: `Kept in ${file}, under [policy], which your team sees.`,
    settings: [
      {
        id: `${POLICY}.standing`,
        label: "Template and families",
        writes: "this machine",
        help: "Change it in the project's settings file and commit it. A tightening applies at once.",
        useControl: () => ({
          grouped: true,
          control: (ids) => <PolicyStanding plane={plane} ids={ids} />,
        }),
      },
      {
        id: `${POLICY}.blocked`,
        label: "Would have blocked",
        writes: "nothing",
        help: "What a family set to log let through, in every chat here: each would have been blocked under enforce.",
        useControl: () => ({
          control: (ids) => <WouldHaveBlocked plane={plane} ids={ids} />,
        }),
      },
    ],
  };
}

/** The enforcement log's page, under Policy. */
export function policyLogGroup(plane: PlaneId): SettingsGroup {
  return {
    id: POLICY_LOG,
    label: "Enforcement log",
    help: "Each block a family set to log let through, newest first: the rule, the family, the chat and the host:port it was about, never the command, a path or a value.",
    sub: true,
    settings: [
      {
        id: `${POLICY_LOG}.lines`,
        label: "Blocks let through",
        writes: "nothing",
        help: "Set a family to enforce in the project's settings file to block these.",
        useControl: () => ({
          control: (ids) => <PolicyLogList plane={plane} ids={ids} />,
        }),
      },
    ],
  };
}
