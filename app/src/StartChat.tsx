// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { UserRound } from "lucide-react";
import { PersonaMark } from "./PersonaMark";
import { HarnessSummary } from "./HarnessCard";
import { FirstOnHarness, noteHarness } from "./cardLink";
import { ApprovalSentence } from "./ProfileApproval";
import {
  commands,
  type PlaneId,
  type ProfileRow,
  type StartOptions,
  type WithoutSandbox,
} from "./bindings";
import { useChatsHere } from "./chatState";
import { profileCommand, profilePage } from "./settings/profileAddress";
import { Button } from "./ui/button/Button";
import { Checkbox } from "./ui/choice/Checkbox";
import { OptionCard, OptionCards } from "./ui/choice/Radio";
import { Dialog } from "./ui/dialog/Dialog";
import { TextField } from "./ui/field/Field";
import { submits as chordSends } from "./ui/field/submitKey";
import { HarnessIcon, ownIconsOf } from "./ui/harness-icon/HarnessIcon";
import { ListBox, ListOption } from "./ui/list/List";
import { FILTERED_FROM, ListFilter, noMatchSaid, type Noun } from "./ui/list/ListFilter";
import { RowLabel, RowLead, RowTrail } from "./ui/list/Row";
import { reachableWords, startAnswer } from "./StartChatApproval";

/**
 * The key of the row that stands for "no persona at all".
 *
 * A list's keys are strings and one of them has to mean nothing. A persona's name starts with a
 * lowercase letter or a digit (`purlis_core::contain::persona_name_ok`), so a key that starts
 * with a colon can never be a real persona's the way a sentinel like `none` could.
 */
const NO_PERSONA = ":none";

/** What the persona list's items are called, for its filter (R11, R15). */
const PERSONAS: Noun = { one: "persona", many: "personas" };

/** The fixed height of the dialog, list beside details: whatever the list holds (R4). */
const HEIGHT = 620;

/**
 * What a new chat asks before anything runs.
 *
 * **No harness starts until somebody picks a profile** (ADR 0022). It shows even when one
 * profile is available, because skipping it would bring back the harness nobody picked on a
 * one-harness machine, and one profile costs one Enter. Escape closes it having started
 * nothing — no harness ran, and no identity was recorded.
 *
 * **Two panes, at a fixed height** (#2918, prototype B of #2798; R1–R36 of the audit in #2897):
 * a wide dialog of the family, `min(620 px, the window less 48 px)` tall whatever the list holds,
 * with the persona list on the left under its filter and what the chat starts with on the right.
 * Each pane scrolls on its own; the header and the footer, with Start, never leave. The dialog is
 * as wide as its size and anchored at a fixed top, so a pick changes nothing of its place.
 *
 * - **The persona list** (`ui/list`): a filter over it once it is long (R10–R16, R164), a row
 *   per persona with its mark (R19), the project's default chipped (R20), and a last row for a
 *   chat with no persona. ↑ and ↓ in the filter move the pick and stop at the ends (R16); a
 *   pick that the filter leaves out goes to the first match (R13). The picked row is scrolled
 *   into view as the dialog opens and as it is picked, never while the person types (R22).
 * - **The persona picked**, at the top of the right pane: its mark, its name and its one line
 *   (R23).
 * - **The harness**: option cards (`ui/choice`), every one the height of the tallest, its icon,
 *   its name and its profile's facts, and no radio circle; the chosen one by its wash, its edge
 *   and its check (R24–R27, #2798 d3).
 * - **The name, the footer box** and, where they apply, the branch box and the sandbox.
 * - **The footer** says what Start will do, `steward on Claude Code` (R33), then Cancel and
 *   Start chat. It keeps the room of every word it can reach in this dialog (R163).
 *
 * **⏎ starts from the filter, a persona row or a harness card** (R17), as ⌘⏎ does from anywhere
 * (N26): never from the Name field, where Return is a stray key, and never an approval or a start
 * without the sandbox, which are a press of their own button (ADR 0097 §6). It starts as the key
 * comes up, once, and a held key's repeats start nothing. The keyboard starts on Cancel, so a
 * Return pressed by habit as the dialog opens starts nothing, and a double-click on a row only
 * picks it.
 *
 * **What stops a start is said at the top of the details**: a refusal, its way to the Inbox,
 * the command to approve, brought into view, never left under the fold of the scrolling pane.
 *
 * A dialog rather than a pane that draws itself: the tmux frame put the selector in the
 * chat's own pane because tmux had already made the pane, and here there is no chat yet.
 * Nothing is opened until a row is picked, so cancelling leaves nothing to tear down.
 *
 * A profile whose command charter has not recorded running shows that command and asks. The
 * file is gitignored, so an edit to it leaves no diff for a reviewer to catch, and nothing
 * stops a chat editing plane config — which is why the ask is about the words that are
 * about to run and not about the profile's name. **No profile icon is drawn beside that
 * approval**: the footer's summary drops the harness's icon while Start is an approval (#2922).
 *
 * **The footer checkbox is asked here and nowhere else** (ADR 0029). Inside a pane
 * charter prints an empty line where its own footer would go, because the app's panels
 * already draw the plane — and this is where an operator says "not this chat". The choice is
 * between charter's footer and nothing: `charter statusline` IS Claude Code's `statusLine`
 * command, so a blank line there leaves the harness with no status line at all.
 *
 * It is asked at the start rather than offered as a switch on a running pane because that
 * command inherits the environment its harness was exec'd with: a toggle on a live chat would
 * appear to work and would not.
 *
 * **The Name field is optional** (charter-app#254). Empty is the default — the persona and the
 * chat's number, `steward 3` — and a name typed here is what the tab says instead. It is
 * charter's label for the chat, not the harness's own name, and the core is what refuses one
 * it will not draw: its refusal comes back into this dialog, before anything has started.
 *
 * **A chat that starts in a repo starts on a new branch in it, unless the box is cleared**
 * (GL-1, ADR 0072 §4). `repo` names the repo whose clone this chat would start in, and only
 * then is the box drawn — ticked, because two chats writing in one working tree is the case
 * this exists to end. Cleared, the chat works on whatever branch the repo has checked out,
 * shared with every other chat there. The core cuts the branch and takes it back if the start
 * is refused, so cancelling here or being refused leaves nothing behind. Not remembered between
 * chats, for the footer box's reason: a box that silently stayed cleared would be a setting.
 *
 * **"Start without the sandbox" is asked here, for this one chat** (ADR 0067 §7, ruling V78 a).
 * Shown only where the project turned the sandbox on, beside what the sandbox does for the
 * picked profile: on, or the reason it cannot be applied here — a missing program, with the
 * distribution's install command (SD-30), or a system with no backend. Where it cannot be
 * applied, the one way on is to start without it, said on the button itself. Not remembered
 * between chats, for the footer box's reason, and never offered by anything but this picker:
 * no CLI word, file or chat makes one, and charter records who turned it off and why. Where an
 * administrator's policy forbids the opt-out (#1343), no box is drawn and the picker says who
 * locked it; a chat the sandbox cannot hold here then does not start.
 *
 * **An arrow key picks the harness it moves to**, as a radio does (the option cards are one
 * radio group). A pick here starts nothing — Start does.
 *
 * **Picking a persona picks its own profile** (#1445). A persona's definition may name the
 * profile its chats start on, and the persona's line says so; picking that persona moves the
 * harness there, and picking one that names none moves it back to the default. **A harness the
 * person picked is never moved**: once they have touched the harness cards, or something asked
 * for a harness (`prefer`), picking a persona changes the persona and nothing else. The name
 * typed and the footer box survive a change of persona too (R32).
 */
export function StartChat({
  options,
  repo,
  place,
  starting = false,
  prefer,
  trouble,
  signIn,
  onShowSignIn,
  onStart,
  onApprove,
  onInstall,
  fixing = false,
  onFix,
  onOpenSettings,
  onCancel,
}: {
  options: StartOptions;
  /** The repo whose clone this chat would start in, when it would start in one. */
  repo?: string;
  /** Where the chat starts, by name: its workspace, or the project's (R8). */
  place?: string;
  /** Whether a start from this picker is running: Start says so and cannot be pressed again,
   *  because a second press was a second chat (GL-1). */
  starting?: boolean;
  /**
   * The harness (a profile's `kind`) to start on, when something already knows which one is
   * wanted — a harness started by hand in a shell tab, opened as a chat instead (ADR 0062).
   * The default profile when it is of that kind, else the first that is; the default when none
   * is. It picks a row and starts nothing: the operator still presses Start.
   */
  prefer?: string;
  /** Why the last attempt did not start, if it did not. */
  trouble?: string;
  /** The profile that last attempt was refused on because its harness says it is not signed
   *  in on this device (#2035, ADR 0095 §3): the refusal points to its Ask in the Inbox. */
  signIn?: string;
  /** Opens the Inbox, where the Ask to sign the profile in waits. Absent, the refusal's own
   *  words say where. */
  onShowSignIn?: () => void;
  /** `label` is the Name field, or `null` when it was left empty. `newBranch` is the branch
   *  box, and `false` when there is none to tick. `withoutSandbox` is the person's opt-out for
   *  this chat, with the reason typed, or `null` for a sandboxed start. */
  onStart: (
    profile: string,
    persona: string | null,
    showFooter: boolean,
    label: string | null,
    newBranch: boolean,
    withoutSandbox: WithoutSandbox | null,
  ) => void;
  /** The profile, the persona, the footer choice, and the exact line the operator read — so
   *  the approval is for what was on screen and not for whatever the file says by the time
   *  it is clicked — and the Name field, as `onStart` has it. */
  onApprove: (
    profile: string,
    persona: string | null,
    showFooter: boolean,
    shown: string,
    label: string | null,
    newBranch: boolean,
    withoutSandbox: WithoutSandbox | null,
  ) => void;
  /** SD-30's install action: the window opens a shell tab at the project root with the
   *  install command typed and not run (ruling V78 c). Absent, the command is only shown. */
  onInstall?: () => void;
  /** Whether a fix pressed here is running: its button says so and cannot be pressed again. */
  fixing?: boolean;
  /** Applies a doctor fix by its id — the `local-ignore` the refusal offers (NO-8) — and reads
   *  the picker again. Absent, the refusal shows the fix's words alone. */
  onFix?: (id: string) => void;
  /** Opens Settings at a group (SE-22's address), with the setting to focus there: where a
   *  refused profile is mended (NO-8, #1296). */
  onOpenSettings?: (group: string, setting?: string) => void;
  onCancel: () => void;
}) {
  // Only a persona there is a row for. The core already filters `[persona] default`
  // against the personas the plane has, so this should be unreachable from the app — but
  // the alternative, if it ever arrives, is a chat started on a persona the operator can
  // neither see nor change, and refused for it a moment later.
  const [persona, setPersona] = useState<string | null>(
    options.persona !== null && options.personas.includes(options.persona) ? options.persona : null,
  );
  // A persona's own profile (#1445), where its definition names one the project offers and
  // the picker has a row for it.
  const ownProfile = (who: string | null): string | undefined => {
    const own = who === null ? undefined : options.persona_profiles[who];
    return options.profiles.some((p) => p.name === own) ? own : undefined;
  };
  // A harness something already asked for (`prefer`), where there is a row of that kind.
  const [preferred] = useState<string | undefined>(() => {
    const ofKind = options.profiles.filter((p) => prefer !== undefined && p.kind === prefer);
    return (ofKind.find((p) => p.is_default) ?? ofKind[0])?.name;
  });
  // The row the picker is on when no persona's own profile moves it: the default.
  const [resting] = useState<string | undefined>(
    () =>
      preferred ?? options.profiles.find((p) => p.is_default)?.name ?? options.profiles[0]?.name,
  );
  const [profile, setProfile] = useState<string | undefined>(
    // A harness something already asked for wins; then the picked persona's own profile.
    () => preferred ?? ownProfile(persona) ?? resting,
  );
  // Whether the harness is somebody's choice already: the person picked one here, or something
  // asked for a harness. After that a persona never moves it.
  const [harnessChosen, setHarnessChosen] = useState(preferred !== undefined);
  const pickProfile = (name: string) => {
    setHarnessChosen(true);
    setProfile(name);
  };
  // Picking a persona moves the harness to that persona's own profile, and to the row it rests
  // on for a persona that names none, so no harness is left over from a persona that is no
  // longer picked. It picks a row and starts nothing. A harness the person chose stays.
  const pickPersona = (who: string | null) => {
    setPersona(who);
    if (!harnessChosen) setProfile(ownProfile(who) ?? resting);
  };
  // Off, which is the app as it has always behaved: a pane's footer is blank unless
  // this chat asks for it (ADR 0029). Not remembered between chats on purpose —
  // there is no plane-wide or machine-wide setting for it, and a box that silently stayed
  // ticked would be one.
  const [showFooter, setShowFooter] = useState(false);
  // Ticked: a chat that starts in a repo gets a branch of its own by default (GL-1). What is
  // sent is the box AND that there was one, so a chat started outside every repo never asks
  // the core for a branch.
  const [onABranch, setOnABranch] = useState(true);
  const newBranch = repo !== undefined && onABranch;
  // What the Name field says. Sent as typed — the core trims it and holds it to its rule — and
  // as nothing at all when there is nothing in it but spaces, which is "the default".
  const [name, setName] = useState("");
  const label = name.trim() === "" ? null : name;
  const picked = options.profiles.find((p) => p.name === profile);
  // Each profile's own SVG, within the list's budget, for its card's icon (#2922).
  const owns = ownIconsOf(options.profiles, options.own_icons);
  const pickedAt = options.profiles.findIndex((p) => p.name === profile);
  // What the sandbox does for a chat on the picked profile, where the project turned it on.
  const sandbox = picked?.sandbox ?? null;
  // The opt-out box, for a chat the sandbox would hold. Off, and not remembered between chats.
  const [optedOut, setOptedOut] = useState(false);
  // Where the sandbox cannot be applied, the start IS the opt-out, and the button says so.
  const refused = sandbox?.state === "refused";
  const [reason, setReason] = useState("");
  // Policy may forbid the opt-out (#1343): then no box, and a chat the sandbox cannot hold
  // here does not start at all, saying who locked it.
  const locked = sandbox?.locked ?? null;
  const withoutSandbox: WithoutSandbox | null =
    locked === null && ((sandbox?.state === "sandboxed" && optedOut) || refused)
      ? { reason: reason.trim() === "" ? null : reason }
      : null;
  const blockedByPolicy = refused && locked !== null;
  // The words Start can reach in this dialog, and whether one is a grant (B1 of #3378's review):
  // the options are fixed while it is open, so the room kept for them never changes.
  // A mock or an older answer may leave a row's `approval` or `sandbox` out: read as none.
  const canApprove = options.profiles.some((row) => Boolean(row.approval));
  const reach = {
    words: reachableWords({
      canApprove,
      canOptOut: options.profiles.some(
        (row) =>
          row.sandbox != null && row.sandbox.locked === null && row.sandbox.state !== "unsandboxed",
      ),
    }),
    grant: canApprove,
  };
  // The project's Local file, by the name the core read, for the two lines that name it.
  const localFile = useLocalFile(
    useChatsHere().plane,
    options.ignore_fix !== null || options.declares_none,
  );
  // Whether it was left with nothing started — Escape or Cancel. Only then does the keyboard go
  // back to what had it: a start hands it to the new chat, and Settings or the install shell
  // take it where they open.
  const cancelled = useRef(false);
  const leave = () => {
    cancelled.current = true;
    onCancel();
  };
  const start = () => {
    if (picked?.approval) {
      if (picked.harness) noteHarness(picked.harness.name);
      onApprove(picked.name, persona, showFooter, picked.shown, label, newBranch, withoutSandbox);
      return;
    }
    if (!profile) return;
    if (picked?.harness) noteHarness(picked.harness.name);
    onStart(profile, persona, showFooter, label, newBranch, withoutSandbox);
  };
  const approves = Boolean(picked?.approval);
  const unavailable = (approves ? !picked : !profile) || blockedByPolicy;
  // What ⏎ from the list or the cards and ⌘⏎ from anywhere may start: a plain start, never an
  // approval or a start without the sandbox (ADR 0097 §6), and nothing while one is running.
  const byKey = !approves && withoutSandbox === null && !unavailable && !starting;

  // The list, and whether the picked row is to be brought into view after this render (R22).
  const list = useRef<HTMLDivElement>(null);
  const scrollTo = useRef(true);
  // **The persona list and its filter** (R10–R16). The filter is drawn once the list is long
  // (R164); the row for no persona is drawn while nothing is typed, as the list's last.
  const [query, setQuery] = useState("");
  const filtered = options.personas.length >= FILTERED_FROM;
  const typed = filtered ? query.trim().toLowerCase() : "";
  const matching = options.personas.filter((who) => who.toLowerCase().includes(typed));
  const keys = typed === "" ? [...matching, NO_PERSONA] : matching;
  const pickedKey = persona ?? NO_PERSONA;
  const onQuery = (to: string) => {
    setQuery(to);
    // A pick the filter leaves out goes to the first match (R13).
    const now = to.trim().toLowerCase();
    const left = options.personas.filter((who) => who.toLowerCase().includes(now));
    if (left.length > 0 && (persona === null || !left.includes(persona))) pickPersona(left[0]);
  };
  // ↑ and ↓ in the filter move the pick through the rows shown, and stop at the ends (R16).
  const moveFromFilter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    if (keys.length === 0) return;
    const at = keys.indexOf(pickedKey);
    const to = Math.max(0, Math.min(keys.length - 1, at + (event.key === "ArrowDown" ? 1 : -1)));
    const key = keys[to];
    pickPersona(key === NO_PERSONA ? null : key);
    scrollTo.current = true;
  };

  // The picked row is brought into view as the dialog opens and as a row is picked, and not as
  // the person types in the filter (R22).
  useLayoutEffect(() => {
    if (!scrollTo.current) return;
    scrollTo.current = false;
    const row = [...(list.current?.querySelectorAll<HTMLElement>("[data-key]") ?? [])].find(
      (one) => one.dataset.key === pickedKey,
    );
    row?.scrollIntoView?.({ block: "nearest" });
  });

  // **⏎ starts from the filter, a persona row or a harness card** (R17, D-2918-3), as a key the
  // person lets go of: armed on a ⏎ that goes down there — not held, not composing, with no
  // modifier (⌘⏎ is the dialog's own chord) — and pressed as it comes up, so a held Return starts
  // once and never again after a refusal, and none of it reaches the new chat's terminal. Only a
  // plain start: never an approval or a start without the sandbox (ADR 0097 §6).
  const armed = useRef(false);
  const enterDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Enter") return;
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const row = target.closest<HTMLElement>('[role="option"][data-key]');
    const filterBox = target instanceof HTMLInputElement && target.type === "search";
    const card = target instanceof HTMLInputElement && target.type === "radio";
    if (!row && !filterBox && !card) return;
    // ⌘⏎ on a card: the card's own press handling keeps the key from the dialog's chord, so it
    // is answered here, once, on the chord's terms (N26).
    if (card && chordSends(event.nativeEvent)) {
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat && byKey) start();
      return;
    }
    if (event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return;
    event.preventDefault();
    // A held key's repeats neither arm nor disarm: the press is the one that went down first.
    if (event.repeat) return;
    armed.current = false;
    // A ⏎ that commits a composition is the composition's (WebKit says so by keyCode 229).
    if (event.nativeEvent.isComposing || event.keyCode === 229) return;
    // From the filter with nothing it matches, there is nothing shown to start.
    if (filterBox && keys.length === 0) return;
    // A row the keyboard is on and not yet picked is the one ⏎ starts.
    const key = row?.dataset.key;
    if (key !== undefined && key !== pickedKey && keys.includes(key))
      pickPersona(key === NO_PERSONA ? null : key);
    armed.current = true;
  };
  const enterUp = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Enter" || !armed.current) return;
    armed.current = false;
    event.preventDefault();
    if (byKey) start();
  };

  // The filter names the row its ↑ and ↓ picked (`aria-activedescendant`), so a screen reader
  // says the pick as it moves. The row's id is React Aria's, read from the row once it is drawn.
  const panes = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const box = panes.current?.querySelector<HTMLInputElement>('input[type="search"]');
    if (!box) return;
    const row = [...(list.current?.querySelectorAll<HTMLElement>("[data-key]") ?? [])].find(
      (one) => one.dataset.key === pickedKey,
    );
    if (row?.id) box.setAttribute("aria-activedescendant", row.id);
    else box.removeAttribute("aria-activedescendant");
  });

  // A refusal is said where the eye is: the details' top, brought into view (B2 of the review).
  const told = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (trouble) told.current?.scrollIntoView?.({ block: "nearest" });
  }, [trouble]);

  const listId = useId();
  const harnessLabel = useId();
  const nameLabel = useId();
  const whoId = useId();
  const who = persona ?? "No persona";

  // **The persona picked**, said once at the top of the details: what it says of itself
  // (#1460), that it is the project's default, and the profile it starts on (#1445) (R23).
  const own = ownProfile(persona);
  const personaLine =
    persona === null
      ? "The chat adopts no persona: it is the harness alone."
      : [
          persona === options.persona ? "Project default" : undefined,
          options.persona_descriptions?.[persona] ?? undefined,
          own === undefined ? undefined : `its profile is ${own}`,
        ]
          .filter((word) => word !== undefined)
          .join(" · ") || "Persona of this project";

  return (
    <Dialog
      onClose={leave}
      returnFocus={() => cancelled.current}
      title="Start a chat"
      titleId="start-chat"
      // Where it starts, and where its profiles come from (R8).
      subtitle={
        <>
          {place === undefined ? "In this project" : `In ${place}`} ·{" "}
          {options.declares_none ? "purlis's built-in profiles" : "this project's profiles"}
        </>
      }
      size="wide"
      height={HEIGHT}
      className="start-chat"
      // Cancel, and not the first thing in tab order: starting a chat runs a command with
      // nothing between the key and the exec, so it is never what a stray Return key finds.
      initialFocus="cancel"
      // What Start will do: the persona on the harness (R33). No profile icon beside an
      // approval (#2922): it is the grant's own look, and a mark there could pass for purlis's.
      summary={
        picked === undefined ? undefined : (
          <span className="start-chat-summary" data-testid="start-chat-summary">
            {who} on{" "}
            {!approves && (
              <HarnessIcon icon={picked.icon} svg={owns[pickedAt]} kind={picked.kind} />
            )}
            {picked.name}
          </span>
        )
      }
      // Its main answer, a start or the approval of the profile's command: the approval's own
      // file (`StartChatApproval.ts`), so no profile icon is drawn beside a grant.
      main={startAnswer({
        approves,
        withoutSandbox: withoutSandbox !== null,
        starting,
        unavailable,
        reach,
        onPress: start,
      })}
    >
      <div
        className="start-chat-panes"
        ref={panes}
        onKeyDownCapture={enterDown}
        onKeyUpCapture={enterUp}
      >
        <div className="start-chat-personas">
          {filtered && (
            <ListFilter
              noun={PERSONAS}
              total={options.personas.length}
              query={query}
              onQuery={onQuery}
              onKeyDown={moveFromFilter}
              controls={listId}
            />
          )}
          <ListBox
            ref={list}
            id={listId}
            className="start-chat-persona-list"
            aria-label="Persona"
            size="lg"
            selectionMode="single"
            disallowEmptySelection
            selectedKeys={keys.includes(pickedKey) ? [pickedKey] : []}
            onSelectionChange={(chosen) => {
              if (chosen === "all") return;
              const [key] = [...chosen];
              if (key === undefined) return;
              pickPersona(key === NO_PERSONA ? null : String(key));
              scrollTo.current = true;
            }}
            empty={<p className="start-chat-empty">{noMatchSaid(PERSONAS, query)}</p>}
          >
            {keys.map((key) =>
              key === NO_PERSONA ? (
                <ListOption key={key} id={key} textValue="No persona">
                  <RowLead>
                    <span className="start-chat-no-mark" aria-hidden="true">
                      <UserRound />
                    </span>
                  </RowLead>
                  <RowLabel>No persona</RowLabel>
                </ListOption>
              ) : (
                <ListOption key={key} id={key} textValue={key}>
                  <RowLead>
                    <PersonaMark persona={key} named />
                  </RowLead>
                  <RowLabel>{key}</RowLabel>
                  {key === options.persona && (
                    <RowTrail>
                      {/* A mark for the eye, drawn by the stylesheet from its attribute: the
                        row's name is the persona's alone (WCAG 2.5.3), and the details say
                        "Project default" for the persona picked. */}
                      <span
                        className="start-chat-chip"
                        data-said="project default"
                        aria-hidden="true"
                      />
                    </RowTrail>
                  )}
                </ListOption>
              ),
            )}
          </ListBox>
        </div>

        <section className="start-chat-details" aria-labelledby={whoId}>
          <div className="start-chat-who" data-testid="start-chat-who">
            {persona === null ? (
              <span className="start-chat-no-mark" aria-hidden="true">
                <UserRound />
              </span>
            ) : (
              <PersonaMark persona={persona} named />
            )}
            <div>
              <h3 id={whoId}>{who}</h3>
              <p>{personaLine}</p>
            </div>
          </div>

          {/* **What stops the start, at the top, where it is seen** (B2 of #3378's review): a
            refusal, its way to the Inbox, the command to approve and what the harness lacks the
            first time. Below them the pane would scroll them out of sight. */}
          <div ref={told} className="start-chat-told">
            {trouble && (
              <p className="honest mid-turn" role="alert">
                {trouble}
              </p>
            )}
            {trouble && signIn !== undefined && onShowSignIn && (
              <p className="honest">
                {/* One Ask per profile, in the Inbox: its Sign in runs the harness's own sign-in
                  in a shell tab the person drives (#2035). */}
                Sign in {signIn} waits in the Inbox.{" "}
                <Button onPress={onShowSignIn}>Go to the Inbox</Button>
              </p>
            )}
            <ApprovalSentence row={picked} />
            {/* What the picked harness lacks, said once by Start: the first chat on it here
              (#1134, W10). */}
            <FirstOnHarness glance={picked?.harness} />
          </div>

          {/* These two lines are drawn once the file's name is known (a read of a moment), so
            the alert is said once, and with the name. */}
          {options.ignore_fix && localFile !== undefined && (
            <p className="honest mid-turn" role="alert">
              git would carry {localFile}, so every profile it declares is refused until that is
              fixed: <code>{options.ignore_fix}</code>
            </p>
          )}
          {/* The doctor's own fix for this state (NO-8, #1233), the one its `harness profiles`
            row offers, where one ignore line is the whole cure. */}
          {options.ignore_fix_id && onFix && (
            <div>
              <Button
                isDisabled={fixing}
                onPress={() => options.ignore_fix_id && onFix(options.ignore_fix_id)}
              >
                {fixing ? "Adding the ignore line…" : "Add the ignore line"}
              </Button>
            </div>
          )}

          <div className="start-chat-section">
            <h3 className="start-chat-label" id={harnessLabel}>
              Harness
            </h3>
            <OptionCards
              aria-labelledby={harnessLabel}
              value={profile ?? null}
              onChange={pickProfile}
            >
              {options.profiles.map((row, at) => (
                <OptionCard
                  key={row.name}
                  value={row.name}
                  // The profile's own icon (#2922): its glyph, its SVG or its harness's monogram.
                  icon={<HarnessIcon icon={row.icon} svg={owns[at]} kind={row.kind} />}
                  title={row.name}
                  // Its facts, the command first among them: the words about to run are read
                  // beside the card that runs them (V69).
                  detail={<CardFacts row={row} />}
                />
              ))}
            </OptionCards>
            {options.declares_none && !options.ignore_fix && localFile !== undefined && (
              <p className="start-chat-help">
                {/* Said rather than shown as an empty list: a project that declares nothing is
                  the ordinary first state, not a fault, and the built-ins still start. */}
                This project declares no profiles of its own, so these are purlis&apos;s built-ins.
                Declare your own in {localFile}, which stays on this machine.
              </p>
            )}
          </div>
          {/* The picked harness's card, at a glance (HP-19): what it runs at and lacks, before
            anything starts. Its whole card is a tab, opened from the chat's header. */}
          {picked?.harness && <HarnessSummary glance={picked.harness} />}

          <div className="start-chat-section">
            <label className="start-chat-label" htmlFor={nameLabel}>
              Name
            </label>
            <TextField
              id={nameLabel}
              value={name}
              onChange={setName}
              // It follows the persona picked: the name the chat takes when this is left empty.
              placeholder={`${persona ?? picked?.kind ?? "chat"} and its number`}
              help="Left empty, the chat is named after its persona and its number. You can rename it from its tab later."
              autoComplete="off"
            />
          </div>

          {/* The reason for the default and the reason against it both belong on screen: the
            panels repeat most of what the footer says, and the footer says it about THIS
            chat's own workspace. */}
          <Checkbox
            isSelected={showFooter}
            onChange={setShowFooter}
            description="The footer names the chat's workspace, which the panels name only for the focused chat."
          >
            Draw purlis&apos;s footer in this chat
          </Checkbox>

          {repo !== undefined && (
            <Checkbox
              isSelected={onABranch}
              onChange={setOnABranch}
              description={
                `Its own branch and folder, cut from what ${repo} has checked out, so this ` +
                "chat's changes stay apart from other chats'. Named after the chat, or chat-1, " +
                `chat-2 and on. Cleared, it works on the branch ${repo} has checked out, shared ` +
                "with every chat there."
              }
            >
              {`Start on a new branch in ${repo}`}
            </Checkbox>
          )}

          {/* One heading over every state of it, so the sentence about a sandbox that is off
            or cannot be applied is not a stray line under the Name box. */}
          {sandbox !== null && (
            <section className="start-chat-section" aria-labelledby={`${nameLabel}-sandbox`}>
              <h3 className="start-chat-label" id={`${nameLabel}-sandbox`}>
                Sandbox
              </h3>
              {sandbox.state === "sandboxed" && locked === null && (
                <Checkbox
                  isSelected={optedOut}
                  onChange={setOptedOut}
                  description={
                    "This project runs every chat sandboxed: it reaches only the hosts the " +
                    "project allows, and never your vaults. Ticked, this one chat runs without " +
                    "it, its tab says so, and purlis records that you turned it off. Nothing " +
                    "inherits it: a relaunch or a resume asks the sandbox again."
                  }
                >
                  Start without the sandbox
                </Checkbox>
              )}
              {/* Sandboxed, with what the picker could not check yet: a profile nobody has
                approved is not run, not even to ask its version (ruling V87g). */}
              {sandbox.state === "sandboxed" && sandbox.said !== "" && (
                <p className="honest">{sandbox.said}</p>
              )}
              {sandbox.state === "unsandboxed" && <p className="honest">{sandbox.said}</p>}
              {refused && (
                <>
                  <p className="honest mid-turn" role="alert">
                    {sandbox.said}
                  </p>
                  {sandbox.install !== null && (
                    <p className="honest">
                      Install it with <code>{sandbox.install}</code>. It needs sudo, so purlis types
                      it in a shell tab and leaves running it to you.{" "}
                      {onInstall && <Button onPress={onInstall}>Type it in a shell tab</Button>}
                    </p>
                  )}
                </>
              )}
              {locked !== null && <p className="honest">{locked}</p>}
              {withoutSandbox !== null && (
                <TextField
                  label="Why, if you want it recorded"
                  value={reason}
                  onChange={setReason}
                  help="Optional. Kept on this machine with the record that this chat ran without the sandbox."
                />
              )}
            </section>
          )}

          {options.refused.length > 0 && (
            <details className="refused">
              {/* A missing profile is a card that is not there — easy to miss in a way a
              missing panel is not — so the ones purlis will not use say why. */}
              <summary>{options.refused.length} refused</summary>
              <ul>
                {options.refused.map(([name, why]) => (
                  <li key={name}>
                    <span>{name}</span> {why}
                    {/* Where the profile is mended (NO-8, #1296): its own page, its command
                    focused. One with no page (a committed profile, a whole file) lands on
                    Harness & profiles, the group its page would be under. */}
                    {onOpenSettings && (
                      <>
                        {" "}
                        <Button
                          onPress={() => onOpenSettings(profilePage(name), profileCommand(name))}
                        >
                          Open {name} in Settings
                        </Button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            </details>
          )}
        </section>
      </div>
    </Dialog>
  );
}

/**
 * **A harness card's facts, in two lines** (R25, R28): what the chat is billed to and the model,
 * then the command that runs — never cut from the card, since it is what is approved (V69) — with
 * where it was declared, whether it is the default and whether it asks for approval. The harness's
 * kind is said only where the profile's name does not say it already.
 */
function CardFacts({ row }: { row: ProfileRow }) {
  const bill = [
    row.billed_to ? `Billed to ${row.billed_to}.` : undefined,
    row.model ? `Model: ${row.model}.` : undefined,
  ].filter((said) => said !== undefined);
  return (
    <span className="start-chat-facts">
      {bill.length > 0 && <span className="start-chat-bill">{bill.join(" ")}</span>}
      <span className="meta profile-meta">
        {row.kind !== row.name && <span className="what">{row.kind}</span>}
        {/* Already contained by the core: a profile is a file a chat can write, and a control
            byte in a command must never redraw this card. */}
        <code className="where">{row.shown}</code>
        <span className="from">{row.source}</span>
        {row.is_default && <span className="what">default</span>}
        {row.approval && <span className="what needs-approval">{row.approval}</span>}
      </span>
    </span>
  );
}

/**
 * **The project's Local settings file, by the name the core read** (#1719, #1340): the file a
 * project declares its own profiles in is `purlis.local.toml` or `charter.local.toml`, and the
 * picker names the one this project has, as Settings does (`settings/project.ts`, `namesOf`).
 *
 * Read from `project_settings` only while a line names the file (`wanted`), so an ordinary start
 * asks the core nothing more. `undefined` until it answers; then the name as code, or, where it
 * could not be read (no project above the picker, or a core that answered nothing usable), what
 * the file is for, never a name guessed.
 */
function useLocalFile(plane: PlaneId | undefined, wanted: boolean): ReactNode | undefined {
  const [read, setRead] = useState<{ plane: PlaneId; file: string | null }>();
  useEffect(() => {
    if (!wanted || plane === undefined) return;
    let gone = false;
    const settle = (file: string | null) => {
      if (!gone) setRead({ plane, file });
    };
    void commands
      .projectSettings(plane)
      .then((answer) => settle(answer.status === "ok" ? (answer.data?.local?.file ?? null) : null))
      .catch(() => settle(null));
    return () => {
      gone = true;
    };
  }, [plane, wanted]);
  if (!wanted) return undefined;
  const file = plane === undefined ? null : read?.plane === plane ? read.file : undefined;
  if (file === undefined) return undefined;
  return file === null ? "the project's local settings file" : <code>{file}</code>;
}
