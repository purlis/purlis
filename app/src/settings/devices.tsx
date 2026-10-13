import { useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { commands, type PendingOffer, type RemoteStatus } from "../bindings";
import { onAMac } from "../tabKeys";
import { useWhileShown } from "../whileShown";
import { encode, toSvgPath } from "../qr";
import { Choice, Field } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Devices** (iOS design §4.2, §6.4): phone access on this machine — whether it listens
 * and where, the paired phones, pairing a new one, and keeping the machine awake while chats
 * run. Nothing here is a project's: it is the machine store's `gateway` and `keep_awake` keys
 * and `peers.json` beside it.
 *
 * **One read, shared by the group's rows** (`remote_status`), read again after every action,
 * on focus, and once a second while a pairing window is open (the offer a phone makes arrives
 * in it). Each row's last refusal is said in that row, in the core's words.
 *
 * **Pairing is the window's to confirm** (ADR 0090): the QR code pairs nothing by itself; a
 * phone that proves the code is offered here, by name and key fingerprint, with Pair and
 * Cancel. The window on the host lasts five minutes whatever this page shows; Stop pairing
 * closes it sooner.
 */
export function devicesGroup(): SettingsGroup {
  return {
    id: "you.devices",
    label: "Devices",
    help: "A phone paired with this machine steers its chats over your network, straight to this machine: on your Wi-Fi, or over Tailscale. Nothing goes through anyone else.",
    settings: [access, addresses, phones, pair, awake],
  };
}

/** How long a pairing code lives on the host (`purlis_core::remote::pairing::WINDOW`). */
export const PAIRING_MINUTES = 5;

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; status: RemoteStatus };

/** A pairing window this page opened: the QR's text and when it was opened. */
type Pairing = { text: string; openedAt: number };

type Held = {
  read: Read;
  /** Why a row's last action was refused, by setting. */
  refused: Readonly<Record<string, readonly string[]>>;
  pairing: Pairing | undefined;
};

const START: Held = { read: { state: "reading" }, refused: {}, pairing: undefined };
let held: Held = START;
const listeners = new Set<() => void>();
/** The newest read out: an answer to an older one is dropped. */
let reading = 0;

function set(change: (was: Held) => Held) {
  held = change(held);
  for (const one of listeners) one();
}

async function reread() {
  const mine = ++reading;
  const said = await commands
    .remoteStatus()
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (mine !== reading) return;
  set((was) => ({
    ...was,
    // A core that answered nothing (an older one, or none at all) is said as trouble, never
    // drawn as a machine with no phones and nothing listening.
    read:
      said.status === "error"
        ? { state: "trouble", trouble: said.error }
        : typeof said.data === "object" && said.data !== null
          ? { state: "read", status: said.data }
          : {
              state: "trouble",
              trouble: "purlis did not say what this machine's phone access is.",
            },
  }));
}

/** Forget a pairing window this page opened: a test's start, never the window's. */
export function forgetPairing(): void {
  held = { ...held, pairing: undefined };
}

function cameBack() {
  if (document.visibilityState === "hidden") return;
  void reread();
}

/** Something elsewhere changed phone access: read again if the group is up. */
export function devicesChanged(): void {
  if (listeners.size > 0) void reread();
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = { ...START, pairing: held.pairing };
    void reread();
    window.addEventListener("focus", cameBack);
    document.addEventListener("visibilitychange", cameBack);
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size > 0) return;
    window.removeEventListener("focus", cameBack);
    document.removeEventListener("visibilitychange", cameBack);
  };
}

/** What a command answered: done, or why not. */
type Done = { status: "ok" } | { status: "error"; error: string };

/** Every action, one at a time, each followed by a read. */
let queue: Promise<void> = Promise.resolve();

function acting(setting: string, act: () => Promise<Done>) {
  const work = async () => {
    const why = await act()
      .then((done) => (done.status === "error" ? done.error : undefined))
      .catch((err: unknown) => String(err));
    set((was) => {
      const refused = { ...was.refused };
      if (why === undefined) Reflect.deleteProperty(refused, setting);
      else refused[setting] = [why];
      return { ...was, refused };
    });
    await reread();
  };
  queue = queue.then(work, work);
}

function useDevices(setting: string) {
  const now = useSyncExternalStore(subscribe, () => held);
  return { read: now.read, error: now.refused[setting], pairing: now.pairing };
}

/** What stands in for a control until the status has been read, or why it could not be. */
function waiting(read: Read): ReactNode | undefined {
  if (read.state === "reading") return <p className="ui-setting-help">Reading…</p>;
  if (read.state === "trouble")
    return (
      <p className="ui-setting-help" role="alert">
        {read.trouble}
      </p>
    );
  return undefined;
}

/** A key fingerprint in groups of four, as the phone shows it, so the two can be read against
 *  each other. */
export function spaced(fingerprint: string): string {
  return fingerprint.replace(/(.{4})(?=.)/g, "$1 ");
}

const access: LiveSetting = {
  id: "you.devices.access",
  label: "Phone access",
  help: "Listens for your paired phones on this machine's network address, and on the addresses below. Off, no phone can connect. It does not reach past your network: pair over Wi-Fi, or over Tailscale.",
  useControl: function useAccess() {
    const id = "you.devices.access";
    const { read, error } = useDevices(id);
    const status = read.state === "read" ? read.status : undefined;
    return {
      error,
      control: (ids) => (
        <>
          <Choice
            kind="toggle"
            ids={ids}
            checked={status?.on ?? false}
            disabled={status === undefined}
            onCheckedChange={(to) => acting(id, () => commands.remoteSetOn(to))}
          />
          <p className="ui-setting-help ui-devices-standing">
            {status === undefined
              ? "Reading…"
              : status.bound.length > 0
                ? `Listening on ${status.bound.join(", ")}.`
                : "Not listening."}
          </p>
        </>
      ),
    };
  },
};

const addresses: LiveSetting = {
  id: "you.devices.addresses",
  label: "Also listen on",
  help: "One address per line, with a port after a colon where it is not the default: a Tailscale address, say. An address that means every interface is skipped. It applies the next time access starts, or now while it is on.",
  useControl: function useAddresses() {
    const id = "you.devices.addresses";
    const { read, error } = useDevices(id);
    const onDisk = read.state === "read" ? read.status.addresses.join("\n") : "";
    const [draft, setDraft] = useState<string | undefined>(undefined);
    const commit = () => {
      if (draft === undefined) return;
      const lines = draft
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line !== "");
      setDraft(undefined);
      if (lines.join("\n") === onDisk) return;
      acting(id, () => commands.remoteSetAddresses(lines));
    };
    return {
      error,
      control: (ids) => (
        <Field
          ids={ids}
          kind="list"
          value={draft ?? onDisk}
          minRows={2}
          onChange={setDraft}
          onCommit={commit}
        />
      ),
    };
  },
};

const phones: LiveSetting = {
  id: "you.devices.phones",
  label: "Paired phones",
  help: "Each phone that may steer this machine's chats. Unpair takes it off and closes its link at once; pair it again to let it back.",
  useControl: function usePhones() {
    const id = "you.devices.phones";
    const { read, error } = useDevices(id);
    return {
      grouped: true,
      error,
      control: (ids) => {
        const instead = waiting(read);
        if (instead !== undefined || read.state !== "read") return instead;
        const { peers } = read.status;
        return (
          <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
            {peers.length === 0 ? (
              <p className="ui-setting-help">No phone is paired yet. Pair one below.</p>
            ) : (
              <ul className="ui-machine-list">
                {peers.map((peer) => (
                  <li key={peer.device} className="ui-machine-entry">
                    <span className="ui-machine-name">{peer.name}</span>
                    <span className="ui-machine-path">{spaced(peer.fingerprint)}</span>
                    <button
                      type="button"
                      className="ui-setting-reset ui-button"
                      // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
                      tabIndex={0}
                      aria-label={`Unpair ${peer.name}`}
                      onClick={() => acting(id, () => commands.remoteUnpair(peer.device))}
                    >
                      Unpair
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        );
      },
    };
  },
};

/** The time left on a pairing window, as `m:ss`, never below 0:00. */
export function timeLeft(openedAt: number, now: number): string {
  const left = Math.max(0, PAIRING_MINUTES * 60 - Math.floor((now - openedAt) / 1000));
  return `${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`;
}

function QrCode({ text }: { text: string }) {
  // Encoded once per code, not once per tick of the clock beside it.
  const code = useMemo(() => encode(text, "M"), [text]);
  return (
    <svg
      className="ui-devices-qr"
      role="img"
      aria-label="Pairing code for your phone"
      viewBox={`0 0 ${code.size} ${code.size}`}
      shapeRendering="crispEdges"
    >
      <path d={toSvgPath(code)} fill="currentColor" />
    </svg>
  );
}

/** The window's own question on a phone's offer (iOS design §4.2 step 3). */
export function Offer({ offer, onDone }: { offer: PendingOffer; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  // One answer per offer, read synchronously: Radix closes the dialog on Pair as well, and
  // that close must not become a Cancel behind the confirmation.
  const answered = useRef(false);
  const answer = (confirm: boolean) => {
    if (answered.current) return;
    answered.current = true;
    setBusy(true);
    acting("you.devices.pair", async () => {
      const done = confirm
        ? await commands.remotePairConfirm(offer.ticket)
        : await commands.remotePairCancel(offer.ticket);
      if (confirm && done.status === "ok") onDone();
      return done;
    });
  };
  return (
    <AlertDialog.Root
      open
      onOpenChange={(open) => {
        if (!open) answer(false);
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="ui-scrim" />
        <AlertDialog.Content className="ui-dialog warning">
          <AlertDialog.Title>Pair {offer.name}?</AlertDialog.Title>
          <AlertDialog.Description>
            Its key fingerprint is <code>{spaced(offer.fingerprint)}</code>. Check it against the
            phone's screen; pair only when the two match.
          </AlertDialog.Description>
          <div className="ui-setting-actions">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="ui-setting-reset ui-button"
                // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
                tabIndex={0}
                disabled={busy}
                onClick={() => answer(false)}
              >
                Cancel
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                className="ui-setting-reset ui-button"
                tabIndex={0}
                disabled={busy}
                onClick={() => answer(true)}
              >
                Pair
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

const pair: LiveSetting = {
  id: "you.devices.pair",
  label: "Pair a phone",
  help: "Shows a code for purlis on your phone to scan. It works for 5 minutes, once, and the phone is paired only after you confirm it here by its name and key fingerprint.",
  useControl: function usePair() {
    const id = "you.devices.pair";
    const { read, error, pairing } = useDevices(id);
    const status = read.state === "read" ? read.status : undefined;
    const [now, setNow] = useState(() => Date.now());
    const [copied, setCopied] = useState<string | undefined>(undefined);
    const expired = pairing !== undefined && now - pairing.openedAt >= PAIRING_MINUTES * 60_000;
    // While a code is live: the status once a second (the offer arrives in it) and the clock.
    useWhileShown(
      1000,
      () => {
        setNow(Date.now());
        void reread();
      },
      pairing !== undefined && !expired,
    );
    const stop = () => {
      set((was) => ({ ...was, pairing: undefined }));
      setCopied(undefined);
      acting(id, () => commands.remotePairStop().then(() => ({ status: "ok" as const })));
    };
    const copy = (text: string) => {
      const clipboard = navigator.clipboard;
      if (clipboard === undefined) {
        setCopied("This window cannot reach the clipboard.");
        return;
      }
      clipboard.writeText(text).then(
        () => setCopied("Copied."),
        () => setCopied("The clipboard refused it."),
      );
    };
    const start = () =>
      acting(id, async () => {
        const started = await commands.remotePairStart();
        if (started.status === "ok")
          set((was) => ({ ...was, pairing: { text: started.data, openedAt: Date.now() } }));
        return started;
      });
    return {
      // A group of controls, not one: the row's name is plain text, and the button keeps its
      // own ("Pair a phone…"), so a `<label for>` never renames it.
      grouped: true,
      error,
      control: (ids) => (
        <div
          className="ui-devices-pairing"
          role="group"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
        >
          {pairing === undefined || expired ? (
            <button
              type="button"
              className="ui-setting-reset ui-button"
              tabIndex={0}
              onClick={start}
            >
              Pair a phone
            </button>
          ) : (
            <>
              <QrCode text={pairing.text} />
              <p className="ui-setting-help">
                Scan it with purlis on your phone. The code works for 5 minutes, once:{" "}
                {timeLeft(pairing.openedAt, now)} left.
              </p>
              <div className="ui-setting-actions">
                <button
                  type="button"
                  className="ui-setting-reset ui-button"
                  tabIndex={0}
                  onClick={() => copy(pairing.text)}
                >
                  Copy pairing text
                </button>
                <button
                  type="button"
                  className="ui-setting-reset ui-button"
                  tabIndex={0}
                  onClick={stop}
                >
                  Stop pairing
                </button>
              </div>
              {copied !== undefined && <p className="ui-setting-help">{copied}</p>}
            </>
          )}
          {expired && (
            <p className="ui-setting-help">The last code ran out. Pair a phone shows a new one.</p>
          )}
          {pairing !== undefined && status?.pending && (
            // Keyed by the ticket: a second offer behind the first is a fresh question, with
            // its own buttons live.
            <Offer
              key={status.pending.ticket}
              offer={status.pending}
              onDone={() => set((was) => ({ ...was, pairing: undefined }))}
            />
          )}
        </div>
      ),
    };
  },
};

const awake: LiveSetting = {
  id: "you.devices.awake",
  label: `Keep this ${onAMac() ? "Mac" : "machine"} awake while chats are running`,
  help: "While a chat is working or waiting on you, this machine does not go to sleep on its own; the display still may. Off, it sleeps as it always did, and a phone away from it loses the link.",
  useControl: function useAwake() {
    const id = "you.devices.awake";
    const { read, error } = useDevices(id);
    const status = read.state === "read" ? read.status : undefined;
    return {
      error,
      control: (ids) => (
        <Choice
          kind="toggle"
          ids={ids}
          checked={status?.keep_awake ?? false}
          disabled={status === undefined}
          onCheckedChange={(to) => acting(id, () => commands.remoteSetKeepAwake(to))}
        />
      ),
    };
  },
};
