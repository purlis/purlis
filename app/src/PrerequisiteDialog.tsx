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
 * What is stripped from free text: control, format and bidi characters (`\p{Cc}`, `\p{Cf}`, the
 * line and paragraph separators), the letters that draw nothing (the Hangul fillers, the blank
 * Braille pattern) and every combining mark left once the text is composed, so nothing can turn
 * the line around, hide what follows or draw over a neighbour.
 */
// Marks of no script of their own (Unicode's Inherited: Latin diacritics, overlays such as a
// strike, marks for symbols, variation selectors) and enclosing marks: after NFC such a mark that
// is still loose was not part of a letter, so it only decorates or disguises. Other scripts' marks
// (Devanagari, Tamil and Thai vowel signs, viramas) are their letters' own parts and are kept, so
// two names that differ only in them stay apart.
const UNSEEN =
  /[\p{Cc}\p{Cf}\p{Me}\p{Script=Inherited}\u2028\u2029\u115F\u1160\u3164\uFFA0\u2800]/gu;

/** A run of more than six marks on one letter is cut (Burmese legitimately carries four), so stacked marks never draw over a line. */
const STACKED_MARKS = /(\p{M}{6})\p{M}+/gu;

/** The most characters of a whole line of names, each already cut to {@link INERT_CAP}. */
const LINE_CAP = INERT_CAP * 4;

const cut = (text: string, cap: number): string => {
  const chars = [...text];
  return chars.length > cap ? `${chars.slice(0, cap - 1).join("")}…` : text;
};

/**
 * **Free text from the file, drawn inert** (ADR 0130 §4): composed (NFC, so an accented letter
 * keeps its accent), stripped of {@link UNSEEN}, then each name in it (a run between spaces) cut
 * to {@link INERT_CAP} characters on its own, so a long one never pushes the next out of view.
 */
export function inertWords(text: string): string {
  const stripped = text.normalize("NFC").replace(UNSEEN, "").replace(STACKED_MARKS, "$1");
  const names = stripped.split(" ").map((name) => cut(name, INERT_CAP));
  return cut(names.join(" "), LINE_CAP);
}

/** {@link inertWords}, isolated (`<bdi>`), so right-to-left text never reorders the line around it. */
function Inert({ text }: { text: string }) {
  return <bdi>{inertWords(text)}</bdi>;
}

/** The why-line of `entry`: the effect of each answer, by state. */
export function whyOf(entry: PolicyPrerequisite): string {
  if (entry.reasons.includes("broken"))
    return entry.after_new_key ? `${AFTER_NEW_KEY_WHY} ${BROKEN_WHY}` : BROKEN_WHY;
  return entry.nothing_kept ? SHIPPED_WHY : KEPT_WHY;
}

/**
 * **One plain clause of what a row's loosening does, by its kind** (ADR 0130 §4). The kind is
 * the pair of words the core gives each kind of row (`policypress.rs` `state_of_standing`): a
 * family's mode, the sandbox's level, the merge route held, a dispatch deny, lineage, a
 * protected branch.
 */
export function consequenceOf(row: PolicyWaiting): string {
  if (row.family === "sandbox")
    return row.committed === "off" ? "chats run with no sandbox" : "chats run in the Basic sandbox";
  if (row.accepted === "held") return "chats merge through the forge route";
  if (row.accepted === "protected") return "chats may push to that branch through the forge route";
  if (row.accepted === "denied" && row.committed === "allowed")
    return "a chat may push the branch of a task it dispatched once that task ends";
  if (row.accepted === "denied" && row.committed === "lifted")
    return "that dispatch is no longer refused";
  switch (row.committed) {
    case "log":
      return "what it would refuse runs, and is logged";
    case "off":
      return "what it would refuse runs, and is not logged";
    default:
      return `it runs as ${inertWords(row.committed)}`;
  }
}

/** "a", "a and b", "a, b and c", each in code font. */
function codes(words: readonly string[]) {
  return words.map((one, at) => (
    <span key={at}>
      {at > 0 && (at === words.length - 1 ? " and " : ", ")}
      <code>
        <Inert text={one} />
      </code>
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
              <code>
                <Inert text={row.family} />
              </code>{" "}
              from <Inert text={row.accepted} /> to <Inert text={row.committed} />:{" "}
              {consequenceOf(row)}.
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
  JSON.stringify([
    entry.digest,
    entry.reasons,
    entry.needs_new_key,
    entry.after_new_key,
    entry.nothing_kept,
    entry.policy.waiting,
  ]);

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
 * `latest` is the list last heard, at once, before React draws it: what a press that returns
 * reads, whichever came first.
 */
function usePrerequisites(plane: PlaneId): {
  list: Prerequisites | undefined;
  readAgain: () => Promise<Prerequisites | undefined>;
  latest: () => Prerequisites | undefined;
} {
  const [list, setList] = useState<Prerequisites>();
  const heard = useRef(0);
  const last = useRef<Prerequisites>(undefined);
  const hold = useCallback((now: Prerequisites) => {
    last.current = now;
    setList(now);
  }, []);
  const readAgain = useCallback(async (): Promise<Prerequisites | undefined> => {
    const before = heard.current;
    const answer = await commands.projectPrerequisites(plane).catch(() => undefined);
    if (answer?.status !== "ok" || !drawable(answer.data)) return undefined;
    if (heard.current === before) hold(answer.data);
    return answer.data;
  }, [plane, hold]);
  useEffect(() => {
    const stop = untilStopped(
      listen<Prerequisites>(PREREQUISITES_CHANGED, (event) => {
        if (event.payload?.plane !== plane || !drawable(event.payload)) return;
        heard.current += 1;
        hold(event.payload);
      }),
    );
    // A list that cannot be read raises nothing: the core still holds what it holds.
    void readAgain();
    return stop;
  }, [plane, readAgain, hold]);
  const latest = useCallback(() => last.current, []);
  return { list, readAgain, latest };
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
    // Which refusal it was is read from the entry read again, never from the words: the same
    // digest still needing a new key is the Keychain's refusal; any other is the press's own.
    const now = await onAskAgain();
    const keychain =
      now?.list.some(
        (one) => one.kind === "policy" && one.digest === shown && one.needs_new_key,
      ) === true;
    setSaid(withNewKey && keychain ? KEYCHAIN_REFUSED : done.error);
  };

  return (
    <RaisedAlert
      raised={raised}
      shows={showsOf(entry)}
      title={
        <>
          <code>
            <Inert text={name} />
          </code>
          &apos;s policy needs your answer
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

/**
 * What the Notice after Accept says after the project's name. A running chat's guards read the
 * policy it started with until #7729 (live reload): when that lands, this goes back to "Guards
 * follow it now".
 */
export const ACCEPTED_AFTER =
  "New chats follow it; a chat already running follows it once you restart it.";

/** What the Notice after Accept says, as text. */
export const acceptedSaid = (name: string) => `Policy accepted for ${name}. ${ACCEPTED_AFTER}`;

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
  const { list, readAgain, latest } = usePrerequisites(plane);
  const [accepted, setAccepted] = useState<string>();
  const [noticeShown, setNoticeShown] = useState(false);
  // The accepted entry is held back until the core's list no longer holds it: a later list that
  // opens it again is a new question. Any list that does not hold it clears it, the empty one
  // too, whether it is heard before the press returns or after.
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
              // The core may have emptied the list before the press returned: then nothing is
              // held back, or a later list opening the same digest would never be raised.
              if (latest()?.list.some((held) => held.digest === one.digest) === true)
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
          <p>
            Policy accepted for <bdi>{name}</bdi>. {ACCEPTED_AFTER}
          </p>
        </Notice>
      )}
    </>
  );
}
