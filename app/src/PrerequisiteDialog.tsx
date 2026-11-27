// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the project in front needs from you, asked as an alert the app raises** (#7719; ADR
 * 0130 §4, §8): each open prerequisite of the project's list (`project_prerequisites` and the
 * `prerequisites-changed` event, #7718) is one {@link RaisedAlert} of the dialog family, shown
 * one after another ("1 of 2"), never stacked. The family owns the mechanics (#7728): the settle
 * guard, no silent swap, the wait for a quiet window, focus and its return. This file owns what
 * the policy prerequisite says and what its answers do.
 *
 * **Accept is the window's alone** ({@link mayAcceptPolicy}): it accepts exactly what was shown,
 * by its digest, on the plane the alert was raised for, and a file that moved since accepts
 * nothing. A linked page shows the answer that grants nothing and says where to accept.
 *
 * **The answer that grants nothing is held in this window's memory for now**, against the digest
 * and reasons it was shown for: the core's hold of it for the run, which every window hears, is
 * #7724's. An entry the core already reads as `deferred` is never raised.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  PREREQUISITES_CHANGED,
  type PlaneId,
  type PolicyPrerequisite,
  type PolicyState,
  type PolicyWaiting,
  type Prerequisites,
} from "./bindings";
import { listen, untilStopped } from "./here";
import { Notice } from "./Notice";
import { mayAcceptPolicy, notInForceSaid } from "./ProjectPolicyNotice";
import { holdPolicy } from "./projectPolicy";
import { projectName } from "./projectName";
import { sandboxCommandReturned } from "./sandboxAsked";
import { Button } from "./ui/button/Button";
import { RaisedAlert } from "./ui/dialog/RaisedAlert";

/** The why-line under Broken: what the answer that grants nothing leaves in force. */
export const BROKEN_WHY = "Until you answer, the project's chats work in strict mode.";

/** The why-line under Broken after purlis made a new seal key (#7714), before {@link BROKEN_WHY}. */
export const AFTER_NEW_KEY_WHY =
  "purlis made a new key on this machine; accept this project's policy again.";

/** The why-line under Waiting, where an acceptance is kept. */
export const KEPT_WHY = "Until you accept it, your chats keep the policy you accepted before.";

/** The why-line under Waiting, where nothing was ever kept. */
export const SHIPPED_WHY = "Your chats keep purlis's shipped default (watch) until you accept.";

/** What heads the summary under Broken: computed against strict mode, never the old record. */
export const BROKEN_SUMMARY =
  "purlis cannot vouch for what you accepted before, so this is everything Accept puts in force:";

/** What a Keychain that refused a new key says (Deny, a wrong password, a locked Keychain). */
export const KEYCHAIN_REFUSED =
  "macOS did not let purlis replace its key, so nothing was accepted.";

/** What a linked page says in Accept's place. */
export const ACCEPT_IN_THE_WINDOW = "Accept it from purlis's own window on this machine.";

/** The most characters of a name from the file drawn before it is cut. */
export const INERT_CAP = 64;

/** The answer that grants nothing, by state: Work in strict mode wherever the policy is Broken. */
export const answerThatGrantsNothing = (entry: PolicyPrerequisite): string =>
  entry.reasons.includes("broken") ? "Work in strict mode" : "Keep my current rules";

/** The granting answer: Make a new key and accept where the seal key is foreign (#7714). */
export const grantingAnswer = (entry: PolicyPrerequisite): string =>
  entry.needs_new_key ? "Make a new key and accept" : "Accept";

/**
 * **Free text from the file, drawn inert** (ADR 0130 §4): every control, format and bidi
 * character stripped (`\p{Cc}`, `\p{Cf}`, the line and paragraph separators), so nothing can turn
 * the line around or hide what follows, and cut to {@link INERT_CAP} characters.
 */
export function inertWords(text: string, cap = INERT_CAP): string {
  const stripped = text.replace(/[\p{Cc}\p{Cf}\u2028\u2029]/gu, "");
  const chars = [...stripped];
  return chars.length > cap ? `${chars.slice(0, cap - 1).join("")}…` : stripped;
}

/** The why-line of `entry`: the effect of each answer, by state. */
export function whyOf(entry: PolicyPrerequisite): string {
  if (entry.reasons.includes("broken"))
    return entry.after_new_key ? `${AFTER_NEW_KEY_WHY} ${BROKEN_WHY}` : BROKEN_WHY;
  return entry.nothing_kept ? SHIPPED_WHY : KEPT_WHY;
}

/** One plain clause of what a row's loosening does. */
export function consequenceOf(row: PolicyWaiting): string {
  if (row.family === "sandbox")
    return row.committed === "off" ? "chats run with no sandbox" : "chats run with the floor only";
  switch (row.committed) {
    case "log":
      return "what it would refuse runs, and is logged";
    case "off":
      return "what it would refuse runs, and is not logged";
    case "lifted":
      return "it is no longer denied";
    case "allowed":
      return "it is allowed";
    default:
      return `it runs as ${inertWords(row.committed)}`;
  }
}

/** "a", "a and b", "a, b and c", each in code font. */
function codes(words: readonly string[]) {
  return words.map((one, at) => (
    <span key={at}>
      {at > 0 && (at === words.length - 1 ? " and " : ", ")}
      <code>{inertWords(one)}</code>
    </span>
  ));
}

/**
 * **What Accept would put in force**: the loosenings first, never collapsed, with their count
 * under Waiting; under Broken, everything against strict mode. Then the locks and what does not
 * run without a sandbox. Every name from the file is drawn inert.
 */
export function PolicySummary({ entry }: { entry: PolicyPrerequisite }) {
  const { waiting, locked } = entry.policy;
  const count = waiting.length;
  const head = entry.reasons.includes("broken")
    ? BROKEN_SUMMARY
    : `Accept loosens ${count} ${count === 1 ? "rule" : "rules"}:`;
  const notInForce = notInForceSaid(entry.policy).trim();
  return (
    <div className="prerequisite-summary ui-text-body">
      <p>{head}</p>
      {count > 0 && (
        <ul aria-label="What Accept puts in force">
          {waiting.map((row, at) => (
            // The rows are drawn once per entry, and two may read alike: the place is the key.
            <li key={at}>
              <code>{inertWords(row.family)}</code> from {inertWords(row.accepted)} to{" "}
              {inertWords(row.committed)}: {consequenceOf(row)}.
            </li>
          ))}
        </ul>
      )}
      {locked.length > 0 && (
        <p>
          {codes(locked)} {locked.length === 1 ? "stays" : "stay"} enforced whatever you accept.
        </p>
      )}
      {notInForce !== "" && <p>{notInForce}</p>}
    </div>
  );
}

/** What the alert shows, as one value: a change of any of it while it is up is said. */
const showsOf = (entry: PolicyPrerequisite) =>
  [entry.digest, entry.reasons.join("+"), entry.needs_new_key, entry.after_new_key].join("|");

/** What an answer that grants nothing was given for: the digest and the reasons shown. */
const answeredFor = (entry: PolicyPrerequisite) => `${entry.digest}|${entry.reasons.join("+")}`;

/**
 * Each project's answers that grant nothing this run, in this window, by what they were given
 * for: a change back to one already answered does not raise it again (ADR 0130 §6).
 */
const answeredThisRun = new Map<PlaneId, Set<string>>();

/** Forgets every answer this window holds: for the tests, one window each. */
export function forgetPrerequisiteAnswers(): void {
  answeredThisRun.clear();
}

/** Whether a read is a list to draw: an answer of any other shape says nothing. */
const drawable = (data: unknown): data is Prerequisites =>
  Array.isArray((data as Prerequisites | null | undefined)?.list);

/**
 * **The project's list as the core last told it**: asked once as it mounts, then each
 * `prerequisites-changed` for this plane. An event that came first wins over the ask, which is
 * older. `readAgain` asks once more and answers what it read, as a refused press does.
 */
function usePrerequisites(
  plane: PlaneId,
): [Prerequisites | undefined, () => Promise<Prerequisites | undefined>] {
  const [list, setList] = useState<Prerequisites>();
  const heard = useRef(0);
  const read = useCallback(async (): Promise<Prerequisites | undefined> => {
    const before = heard.current;
    const answer = await commands.projectPrerequisites(plane).catch(() => undefined);
    if (answer?.status !== "ok" || !drawable(answer.data)) return undefined;
    if (heard.current === before) setList(answer.data);
    return answer.data;
  }, [plane]);
  useEffect(() => {
    const stop = untilStopped(
      listen<Prerequisites>(PREREQUISITES_CHANGED, (event) => {
        if (event.payload?.plane !== plane || !drawable(event.payload)) return;
        heard.current += 1;
        setList(event.payload);
      }),
    );
    // A list that cannot be read raises nothing: the core still holds what it holds.
    void read();
    return stop;
  }, [plane, read]);
  return [list, read];
}

/** A press's answer, whatever it came to: a promise that threw is a refusal with its words. */
type Pressed = { status: "ok"; data: PolicyState } | { status: "error"; error: string };

/** **The policy prerequisite's alert** (ADR 0130 §4): one entry, its answers bound to `plane`. */
function PolicyAlert({
  plane,
  entry,
  canAccept,
  onAccepted,
  onAskAgain,
  onOpenPolicy,
}: {
  plane: PlaneId;
  entry: PolicyPrerequisite;
  canAccept: boolean;
  onAccepted: (state: PolicyState) => void;
  /** Reads the core's list again, and answers it. */
  onAskAgain: () => Promise<Prerequisites | undefined>;
  onOpenPolicy?: () => void;
}) {
  const name = projectName(plane);
  const key = answeredFor(entry);
  const [answered, setAnswered] = useState(() => answeredThisRun.get(plane)?.has(key) === true);
  const [setAside, setSetAside] = useState<string>();
  const [said, setSaid] = useState<string>();
  const openPolicyAfter = useRef(false);

  // A new entry is a new question: what was answered or said about the last one is not of it.
  const [was, setWas] = useState(key);
  if (was !== key) {
    setWas(key);
    setAnswered(answeredThisRun.get(plane)?.has(key) === true);
    setSaid(undefined);
  }

  const raised = entry.state === "open" && !answered && setAside !== key;

  const grantNothing = () => {
    let held = answeredThisRun.get(plane);
    if (held === undefined) answeredThisRun.set(plane, (held = new Set()));
    held.add(key);
    setAnswered(true);
  };

  const accept = async () => {
    const shown = entry.digest;
    const withNewKey = entry.needs_new_key;
    setSaid(undefined);
    const press = withNewKey
      ? commands.acceptProjectPolicyWithNewKey
      : commands.acceptProjectPolicy;
    const done: Pressed = await press(plane, shown).catch((err: unknown) => ({
      status: "error" as const,
      error: `purlis could not keep your acceptance: ${String(err)}`,
    }));
    if (done.status === "ok") {
      onAccepted(done.data);
      return;
    }
    // Which refusal it was is read from the list, never from the words: the same digest after
    // a new key's press is the Keychain's refusal, and a digest that moved is the file's.
    const now = await onAskAgain();
    const same = now?.list.some((one) => one.kind === "policy" && one.digest === shown) === true;
    setSaid(withNewKey && same ? KEYCHAIN_REFUSED : done.error);
  };

  return (
    <RaisedAlert
      raised={raised}
      shows={showsOf(entry)}
      title={
        <>
          <code>{inertWords(name)}</code>&apos;s policy needs your answer
        </>
      }
      why={whyOf(entry)}
      summary={canAccept ? undefined : ACCEPT_IN_THE_WINDOW}
      cancel={{ children: answerThatGrantsNothing(entry), onPress: grantNothing }}
      main={
        canAccept
          ? { children: grantingAnswer(entry), variant: "grants", onPress: accept }
          : undefined
      }
      className="prerequisite-alert"
      data-testid="prerequisite-alert"
      afterClose={() => {
        if (!openPolicyAfter.current) return;
        openPolicyAfter.current = false;
        onOpenPolicy?.();
      }}
    >
      <PolicySummary entry={entry} />
      {said !== undefined && (
        <p role="alert" className="ui-text-body">
          {said}
        </p>
      )}
      {onOpenPolicy !== undefined && (
        <Button
          variant="link"
          onPress={() => {
            // Closed unanswered: the prerequisite stays open, and its Notice stands.
            openPolicyAfter.current = true;
            setSetAside(key);
          }}
        >
          Open the policy in Settings
        </Button>
      )}
    </RaisedAlert>
  );
}

/** What the Notice after Accept says. */
export const acceptedSaid = (name: string) =>
  `Policy accepted for ${name}. Guards follow it now; a chat started in strict mode keeps the full sandbox until you restart it.`;

/**
 * **The project's prerequisites, raised** (see above), with the Notice that follows an Accept.
 * Drawn in the project's view: a project behind keeps its alert until it is in front.
 */
export function PrerequisiteDialog({
  plane,
  canAccept = mayAcceptPolicy(),
  onOpenPolicy,
}: {
  plane: PlaneId;
  /** Whether this page may accept: the window may, a link may not. */
  canAccept?: boolean;
  /** Opens Settings › Project › Policy, once the alert has closed unanswered. */
  onOpenPolicy?: () => void;
}) {
  const [list, readAgain] = usePrerequisites(plane);
  const [accepted, setAccepted] = useState<string>();
  const [noticeShown, setNoticeShown] = useState(false);
  // The accepted entry is held back until the core's list no longer holds it: a later list that
  // opens it again is a new question.
  const [seen, setSeen] = useState(list);
  if (seen !== list) {
    setSeen(list);
    if (accepted !== undefined && list?.list.some((one) => one.digest === accepted) !== true)
      setAccepted(undefined);
  }

  const name = inertWords(projectName(plane));
  return (
    <>
      {list?.list.map((one, at) =>
        one.kind === "policy" && one.digest !== accepted ? (
          // One entry per place in the core's order: a new list keeps each one's place.
          <PolicyAlert
            key={at}
            plane={plane}
            entry={one}
            canAccept={canAccept}
            onAccepted={(state) => {
              setAccepted(one.digest);
              setNoticeShown(true);
              holdPolicy(plane, state);
              sandboxCommandReturned();
            }}
            onAskAgain={readAgain}
            onOpenPolicy={onOpenPolicy}
          />
        ) : null,
      )}
      {noticeShown && (
        <Notice
          cause={`policy-accepted:${plane}`}
          tone="news"
          label="The project's policy"
          onDismiss={() => setNoticeShown(false)}
        >
          <p>{acceptedSaid(name)}</p>
        </Notice>
      )}
    </>
  );
}
