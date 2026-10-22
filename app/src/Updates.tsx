// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import * as Dialog from "./overlays/dialog";
import { listen } from "./here";
import { ArrowUpCircle, LoaderCircle, Pin } from "lucide-react";
import {
  commands,
  type Offer,
  type PinReport,
  type PlaneId,
  type RelaunchChoice,
} from "./bindings";
import { MidTurn, mightBeMidTurn, type Ending } from "./QuitWarning";
import { ReleaseNotes } from "./ReleaseNotes";
import { Choice, SettingActions, SettingRow } from "./settings/components";
import { channelMoved, useUpdateChannel } from "./updateChannel";
import { AnswerBar } from "./AnswerBar";
import { Button } from "./ui/button/Button";
import { StatusItem } from "./ui/status/StatusItem";

/**
 * **"An update is available", and the pin that drifts** — the two version facts charter ADR
 * 0038 left with no surface, drawn on the status line.
 *
 * # The update offer
 *
 * The updater (#158, ADR 0042) checks on its own and installs only on a click, and its
 * author named this file's job: listen for `update://checked`, show the offer, call
 * `installUpdate()` from it, and show `updateChannel()` / `setUpdateChannel()` beside it.
 *
 * **What installing costs is said before the click, never after it.** Every session is a child
 * of this process (ADR 0025). On Windows the installer ends charter at once; on macOS and Linux
 * the new version is put in place and runs from the next start — and starting it again means
 * ending this one. Either way the running chats end, and the offer says so.
 *
 * **Once it is installed the line says Restart to update** (charter-app#251), and that is the
 * way to finish. The core writes every project's record, saying a restart to update wrote it,
 * ends the chats and restarts. **What to put back is answered here, before the restart**
 * (#1078): every chat reopened (in front, as the launch's own question has it) or a fresh
 * start. The core carries the answer to the launch after the restart, which then asks
 * nothing; a launch that does not follow it (the restart failed, charter started by hand)
 * asks #250's question as before. **A chat that is mid-turn is named first**, in the words
 * the quit warning uses, with
 * Wait as the answer a stray Return finds. Nothing here restarts charter behind the
 * operator's back.
 *
 * **A check that failed on its own is not drawn.** The timer runs every few hours whether or
 * not the laptop is on a train, and a line that turned amber every time Wi-Fi dropped would be
 * furniture by Friday. A failure is drawn only after the operator asked for something — a
 * check, an install, a channel change.
 *
 * # The pin
 *
 * Only when `charter version` says the plane's `[charter] version` is one this charter does not
 * meet — and that verdict is `adopt::version_report`'s exit status (`app/src-tauri/src/pin.rs`),
 * never a comparison made here (ADR 0030, as amended by ADR 0045). Its dialog carries `charter version`'s own
 * sentences.
 */

/** Where the updater is, as the window knows it. */
export type UpdateState =
  | { kind: "quiet" }
  | { kind: "offered"; offer: Offer }
  | { kind: "installing"; offer: Offer }
  /** `refused` is the core's sentence when a restart to update did not go ahead. */
  | { kind: "installed"; version: string; refused?: string }
  | { kind: "failed"; why: string };

/** What the window knows about updates, and what it can ask for. */
export type Updates = {
  state: UpdateState;
  /** The channel this machine takes charter from, once asked. */
  channel?: string;
  check: () => void;
  install: () => void;
  choose: (channel: string) => void;
  /** Restart into the installed update — once the operator has heard what is mid-turn. With
   *  `after`, what the launch after it puts back, so it asks nothing (#1078). */
  restart: (after?: RelaunchChoice) => void;
};

/**
 * Stops one listener, and never lets that failure escape as an unhandled rejection.
 *
 * **`listen`'s unlisten function is `async`** (`@tauri-apps/api/event`'s `_unlisten`), so
 * calling it without awaiting hands back a promise nobody is holding — and it rejects whenever
 * `__TAURI_EVENT_PLUGIN_INTERNALS__` is not there: a webview being torn down, and every jsdom
 * test whose mocked IPC has no event plugin behind it. The `.catch` further down covers the
 * async block around the awaits; a bare call is outside it.
 *
 * It surfaced the day the updater moved to the window (`TitleBar.tsx`): mounted inside a
 * project it outlived the registration, mounted on the window it is torn down by any test that
 * renders and unmounts quickly — so `gone` was true before `listen` resolved, and the cleanup
 * fired into nothing. There is nothing to do about a listener that cannot be removed from a
 * page that is going away, and an unhandled rejection out of a cleanup function is a real
 * failure in a real window as well as a red suite.
 */
function stopListening(stop: () => void) {
  try {
    void Promise.resolve(stop()).catch(() => {});
  } catch {
    // A synchronous throw from the same cause, and the same answer.
  }
}

/**
 * The updater's events, turned into one state.
 *
 * `asked` is what separates a failure worth drawing from one that is not (see the module doc):
 * it is set by every gesture that asks the updater for something and cleared by the answer.
 *
 * **Called once, on the WINDOW** (`App.tsx`), and drawn once, on the title bar. It used to be
 * called in each `PlaneView`, which made a window holding eight projects hold eight clients
 * for one app-wide fact.
 */
export function useUpdates(): Updates {
  const [state, setState] = useState<UpdateState>({ kind: "quiet" });
  // One value with Settings › You › This machine (#1240), read when the window first asks.
  const channel = useUpdateChannel();
  const asked = useRef(false);

  useEffect(() => {
    let gone = false;
    const stops: (() => void)[] = [];
    void (async () => {
      const on = async <T,>(event: string, then: (payload: T) => void) => {
        const stop = await listen<T>(event, (e) => {
          if (!gone) then(e.payload);
        });
        if (gone) stopListening(stop);
        else stops.push(stop);
      };
      await on<Offer | null>("update://checked", (offer) => {
        asked.current = false;
        setState((was) =>
          // An install under way is not undone by a timer's check landing in the middle of it.
          was.kind === "installing" || was.kind === "installed"
            ? was
            : offer
              ? { kind: "offered", offer }
              : { kind: "quiet" },
        );
      });
      await on<string>("update://installed", (version) => {
        asked.current = false;
        setState({ kind: "installed", version });
      });
      await on<string>("update://failed", (why) => {
        const wasAsked = asked.current;
        asked.current = false;
        if (wasAsked) setState({ kind: "failed", why });
      });
    })().catch(() => {
      // A window that cannot listen draws the quiet updater — the icon, the channel, and
      // `Check now` — rather than taking the status line down. Seen on CI: a test whose mock
      // core throws for every command it does not name made `listen` reject, unhandled.
    });
    return () => {
      gone = true;
      for (const stop of stops) stopListening(stop);
    };
  }, []);

  const check = useCallback(() => {
    asked.current = true;
    void commands.checkForUpdate();
  }, []);

  const install = useCallback(() => {
    setState((was) => (was.kind === "offered" ? { kind: "installing", offer: was.offer } : was));
    asked.current = true;
    void commands.installUpdate();
  }, []);

  const choose = useCallback((next: string) => {
    void commands.setUpdateChannel(next).then((done) => {
      if (done.status === "error") {
        setState({ kind: "failed", why: done.error });
        return;
      }
      channelMoved(next);
      // An offer from the other channel's manifest is not an offer on this one.
      setState({ kind: "quiet" });
      asked.current = true;
      void commands.checkForUpdate();
    });
  }, []);

  const restart = useCallback((after?: RelaunchChoice) => {
    void (
      after === undefined ? commands.restartToUpdate() : commands.restartToUpdateChoosing(after)
    )
      .then((done) => (done.status === "error" ? done.error : undefined))
      // A command that never reached the core is a refusal too, and says so in the same place.
      .catch((why: unknown) => `purlis could not ask to restart: ${String(why)}`)
      .then((refused) => {
        // On success nothing comes back: the process is on its way out.
        if (refused !== undefined)
          setState((was) => (was.kind === "installed" ? { ...was, refused } : was));
      });
  }, []);

  return { state, channel, check, install, choose, restart };
}

/** The words on the line for each state, or none — a quiet updater is an icon and no words. */
function said(state: UpdateState): string | undefined {
  switch (state.kind) {
    case "offered":
      return `${state.offer.version} available`;
    case "installing":
      return `installing ${state.offer.version}`;
    case "installed":
      return "Restart to update";
    case "failed":
      // What to press, as the line can say it: itself, for why. The dialog it opens says
      // the next press, Check now (#1156).
      return "update failed — press to see why";
    case "quiet":
      return undefined;
  }
}

/** The channels a machine can be on, as the Channel row offers them; the one it is on says so. */
function channels(current: string | undefined) {
  return ["stable", "dev"].map((name) => ({
    value: name,
    label: name,
    says: name === current ? "current" : undefined,
  }));
}

/** What the launch after a restart to update puts back, as the update's dialog offers it. */
const AFTER_RESTART: { value: RelaunchChoice; label: string }[] = [
  { value: "ReopenAll", label: "Reopen every chat" },
  { value: "StartFresh", label: "Start fresh" },
];

/** The sentence that has to be read before Install is pressed. */
export const INSTALL_ENDS_SESSIONS =
  "Installing ends every running chat. On Windows purlis closes at once to install; on macOS " +
  "and Linux the new version is put in place, and Restart to update ends every chat and puts " +
  "them all back when purlis starts again, or starts fresh, as you pick.";

/**
 * The status line's update button, and the dialog it opens.
 *
 * `chats` is every chat the window holds, each with its state, as the quit warning is given
 * them — the restart ends them all, so it asks about the ones that are mid-turn across every
 * project, not only the one in front.
 */
export function UpdateItem({
  updates,
  chats = [],
}: {
  updates: Updates;
  chats?: readonly Ending[];
}) {
  const [open, setOpen] = useState(false);
  /** The channel picked and not yet confirmed. A radio's pick follows the arrow keys, and the
   *  channel decides what is installed next, so it is written by **Use this channel** and
   *  never by the pick (DS-3d). Forgotten when the dialog closes. */
  const [picked, setPicked] = useState<string>();
  /** The channel being written, and the update state it was pressed in: a change of state to
   *  a failure after the press is this write's refusal. */
  const [writing, setWriting] = useState<{ to: string; from: UpdateState }>();
  /** The channel the last confirmed write moved this machine to, said until the next pick. */
  const [moved, setMoved] = useState<string>();
  const content = useRef<HTMLDivElement>(null);
  /** Whether the dialog is asking about the chats a restart would interrupt. */
  const [asking, setAsking] = useState(false);
  /** What the launch after the restart puts back (#1078): every chat, unless picked. */
  const [after, setAfter] = useState<RelaunchChoice>("ReopenAll");
  const { state, channel, check, install, choose, restart } = updates;
  // How the write went, read as soon as the props say (React's "adjusting state when a prop
  // changes", so the row never draws a stale pick for a frame).
  if (writing !== undefined && channel === writing.to) {
    // Written: the pick is the channel now. The button it was pressed on is gone, so the
    // keyboard goes to the channel it chose (the effect below), and the move is said.
    setWriting(undefined);
    setPicked(undefined);
    setMoved(writing.to);
  } else if (writing !== undefined && state !== writing.from && state.kind === "failed") {
    // Refused, with the core's reason drawn above: the row shows the channel it is still on.
    setWriting(undefined);
    setPicked(undefined);
  }
  useEffect(() => {
    // Once the group is enabled again, a render after the write landed.
    if (moved !== undefined)
      content.current?.querySelector<HTMLElement>('[role="radio"][data-state="checked"]')?.focus();
  }, [moved]);
  // Live, so a chat that finishes its turn while the ask is up leaves it.
  const midTurn = chats.filter(mightBeMidTurn);
  const words = said(state);
  const toRestart = () => {
    if (midTurn.length === 0) return restart(after);
    setOpen(false);
    setAsking(true);
  };
  const label = words
    ? `Updates: ${words}`
    : `Updates — ${channel ?? "…"} channel, nothing new known`;
  return (
    <>
      <Dialog.Root
        open={open}
        onOpenChange={(to) => {
          setOpen(to);
          setPicked(undefined);
          setMoved(undefined);
        }}
      >
        <Dialog.Trigger asChild>
          <StatusItem
            // A status pill (R115): a quiet updater is its icon alone, in the pill's quiet words;
            // an offer is the one thing there that is news, on the accent's ground; a failure the
            // person asked about is amber beside its words.
            variant={
              state.kind === "offered" || state.kind === "installed" || state.kind === "installing"
                ? "accent"
                : "pill"
            }
            tone={state.kind === "failed" ? "waiting" : undefined}
            data-testid="status-update"
            data-kind={state.kind}
            aria-label={label}
            tooltip={label}
            icon={
              state.kind === "installing" ? (
                <LoaderCircle aria-hidden="true" className="spinning" />
              ) : (
                <ArrowUpCircle aria-hidden="true" />
              )
            }
          >
            {words}
          </StatusItem>
        </Dialog.Trigger>
        <Dialog.Portal>
          <Dialog.Overlay className="ui-scrim" />
          <Dialog.Content
            ref={content}
            className="ui-dialog warning update"
            aria-describedby="update-what"
          >
            <Dialog.Title>Updates</Dialog.Title>
            <div id="update-what">
              {state.kind === "offered" || state.kind === "installing" ? (
                <>
                  <p>
                    purlis <strong>{state.offer.version}</strong> is available on the{" "}
                    {state.offer.channel} channel. This is {state.offer.current}.
                  </p>
                  {/* A stable release's notes are its CHANGELOG.md section, in Markdown. */}
                  {state.offer.notes && (
                    <div className="update-notes">
                      <ReleaseNotes markdown={state.offer.notes} />
                    </div>
                  )}
                  <p className="honest mid-turn" role="alert" data-testid="update-ends-sessions">
                    {INSTALL_ENDS_SESSIONS}
                  </p>
                </>
              ) : state.kind === "installed" ? (
                <>
                  <p className="honest">
                    purlis {state.version} is installed. Restart to update ends every chat and
                    starts purlis {state.version}. Reopen every chat puts each one back, resuming
                    its conversation where its harness can; Start fresh puts none back.
                  </p>
                  {state.refused && (
                    <p className="honest doctor-trouble" role="alert">
                      {state.refused}
                    </p>
                  )}
                </>
              ) : state.kind === "failed" ? (
                <>
                  <p className="honest doctor-trouble" role="alert">
                    {state.why}
                  </p>
                  <p className="honest">Check now tries again.</p>
                </>
              ) : (
                <p className="honest">
                  No newer purlis is known. purlis checks on its own every few hours.
                </p>
              )}
            </div>
            <SettingRow
              label="Channel"
              grouped
              control={(ids) => (
                <Choice
                  ids={ids}
                  kind="radio"
                  options={channels(channel)}
                  value={picked ?? channel}
                  onValueChange={(to) => {
                    setPicked(to);
                    setMoved(undefined);
                  }}
                  disabled={writing !== undefined}
                />
              )}
            />
            {state.kind === "installed" && (
              <SettingRow
                label="After the restart"
                grouped
                control={(ids) => (
                  <Choice
                    ids={ids}
                    kind="radio"
                    options={AFTER_RESTART}
                    value={after}
                    onValueChange={(to) =>
                      setAfter(to === "StartFresh" ? "StartFresh" : "ReopenAll")
                    }
                  />
                )}
              />
            )}
            {picked !== undefined && picked !== channel && (
              <SettingActions>
                <Button
                  isDisabled={writing !== undefined}
                  onPress={() => {
                    if (writing !== undefined) return;
                    setWriting({ to: picked, from: state });
                    choose(picked);
                  }}
                >
                  Use this channel
                </Button>
              </SettingActions>
            )}
            {moved !== undefined && (
              <p className="honest" role="status">
                {`This machine is on the ${moved} channel now.`}
              </p>
            )}
            {/* `tabIndex={0}` on every one, per `docs/ui-primitives.md` (charter-app#186). The
                channel radios above are the scope's first edge and `Close` is its last, which
                left `Install`, `Restart to update` and `Check now` in the middle — where Radix's
                focus scope does nothing and WebKit will not tab to a `<button>` whose
                `tabindex` is not written down. Installing an update was a mouse-only act. */}
            {/* The answer bar's order (`docs/design-system.md`, #1719): the way out first, then
                Check now, and the act that moves things on (Install, Restart to update) last,
                at the trailing edge. */}
            <AnswerBar>
              <Dialog.Close asChild>
                <Button>Close</Button>
              </Dialog.Close>
              {state.kind !== "installing" && state.kind !== "installed" && (
                <Button onPress={check}>Check now</Button>
              )}
              {state.kind === "offered" && (
                <Button onPress={install}>Install {state.offer.version}</Button>
              )}
              {state.kind === "installed" && <Button onPress={toRestart}>Restart to update</Button>}
            </AnswerBar>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      {asking && (
        <MidTurn
          chats={midTurn}
          onWait={() => setAsking(false)}
          onRestart={() => {
            setAsking(false);
            restart(after);
          }}
        />
      )}
    </>
  );
}

/**
 * The project's pin report: once when the project opens, and again whenever it is opened.
 *
 * **A read that fails is kept, not swallowed** (#1719): `trouble` is the core's sentence, so the
 * pin item can say the pin was not read rather than draw nothing, which reads as "no drift".
 * The next read that answers drops it.
 */
export function usePin(plane: PlaneId): {
  pin?: PinReport;
  trouble?: string;
  again: () => void;
} {
  const [pin, setPin] = useState<PinReport>();
  const [trouble, setTrouble] = useState<string>();
  const ask = useCallback(() => {
    void commands
      .planePin(plane)
      .then((answer) => {
        if (answer.status === "error") {
          setTrouble(answer.error);
          return;
        }
        if (answer.data && typeof answer.data.drift === "boolean") {
          setPin(answer.data);
          setTrouble(undefined);
        }
      })
      .catch((err: unknown) => setTrouble(String(err)));
  }, [plane]);
  useEffect(() => ask(), [ask]);
  return { pin, trouble, again: ask };
}

/**
 * The pin item: nothing unless `charter version` says the pin drifts, or the pin could not be
 * read (`trouble`), which it says rather than hiding.
 *
 * **Named by its words** ("Pin 9.0.0", WCAG 2.5.3, as the kill switch is); the sentence that
 * says both versions is its tooltip, its description while it is shown.
 */
export function PinItem({
  pin,
  trouble,
  again,
}: {
  pin?: PinReport;
  trouble?: string;
  again: () => void;
}) {
  const unread = trouble !== undefined && !pin?.drift;
  if (!pin?.drift && !unread) return null;
  const shown = unread ? "Pin unread" : `Pin ${pin?.pinned ?? "(unreadable)"}`;
  const said = unread ? [`purlis could not read the project's pin: ${trouble}`] : (pin?.said ?? []);
  const label = unread
    ? `purlis could not read the project's pin: ${trouble}`
    : `The project pins purlis ${pin?.pinned ?? "(unreadable)"}; this purlis is ${pin?.brought}`;
  return (
    <Dialog.Root onOpenChange={(now) => now && again()}>
      <Dialog.Trigger asChild>
        <StatusItem
          data-testid="status-pin"
          tooltip={label}
          tone="waiting"
          icon={<Pin aria-hidden="true" />}
        >
          {shown}
        </StatusItem>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="ui-scrim" />
        <Dialog.Content className="ui-dialog warning update" aria-describedby="pin-said">
          <Dialog.Title>The project&apos;s pin</Dialog.Title>
          <div id="pin-said">
            {said.map((line) => (
              <p key={line} className="honest">
                {line}
              </p>
            ))}
          </div>
          {/* `tabIndex={0}`, per `docs/ui-primitives.md` (charter-app#186): the one control
              this dialog has, and WebKit leaves a `<button>` out of the tab sequence unless
              its `tabindex` is written down. */}
          <AnswerBar>
            <Dialog.Close asChild>
              <Button>Close</Button>
            </Dialog.Close>
          </AnswerBar>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
