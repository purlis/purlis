// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  commands,
  type BlockedLately,
  type GrantLevel,
  type OpenChat,
  type PlaneId,
  type SandboxNetwork,
} from "../bindings";
import { ALLOW_SAID } from "../allowLabels";
import { Notice } from "../Notice";
import { inTheWindow } from "../personaHostsAllow";
import { useReadOnlySaid } from "../ReadOnlyLine";
import { sandboxCommandReturned, useSandboxCommands } from "../sandboxAsked";
import { Choice, Field, type RowIds } from "./components";

/**
 * **Settings › Sandbox › Network** (#1662, spec #1661): the hosts every sandboxed chat here
 * reaches without asking (**Open hosts**), and what chats here were refused lately (**Blocked
 * lately**), each with Allow. Both are the core's, read from the project's files and this
 * machine's network record; nothing here lists a host of its own. The **Allowed** list beside
 * them is `GrantedList.tsx`'s.
 */

/** How many Allows on a refusals list have returned: the Allowed list beside it reads again
 *  when it moves. A count and not an event, as `sandboxAsked.ts`'s is. */
let allowedHere = 0;
const allowListeners = new Set<() => void>();

function allowedOnAList(): void {
  allowedHere += 1;
  for (const listener of [...allowListeners]) listener();
}

/** How many Allows on a refusals list have returned. */
export function useAllowedHere(): number {
  return useSyncExternalStore(
    (listener) => {
      allowListeners.add(listener);
      return () => void allowListeners.delete(listener);
    },
    () => allowedHere,
    () => allowedHere,
  );
}

/** When, as the lists say it: the day and time. */
export function when(at: number): string {
  return new Date(at * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** What the core said went wrong, through the Notice every surface says it with. */
function Trouble({ said, onDismiss }: { said: string; onDismiss: () => void }) {
  return (
    <Notice cause={`network:${said}`} at="pane" tone="trouble" onDismiss={onDismiss}>
      {said}
    </Notice>
  );
}

/** The page's reading of the core: asked again whenever a sandbox command of the window's
 *  returns, so an Allow here or a Remove beside it shows at once. */
function useNetwork(plane: PlaneId): {
  network: SandboxNetwork | undefined;
  said: string | undefined;
  setSaid: (said: string | undefined) => void;
} {
  const [network, setNetwork] = useState<SandboxNetwork>();
  const [said, setSaid] = useState<string>();
  const moved = useSandboxCommands();
  const asking = useRef(0);
  const read = useCallback(() => {
    const mine = ++asking.current;
    void commands
      .sandboxNetwork(plane)
      .then((done) => {
        if (mine !== asking.current) return;
        if (done.status === "error") setSaid(done.error);
        else setNetwork(done.data);
      })
      .catch(
        (err: unknown) =>
          mine === asking.current &&
          setSaid(`purlis could not read this project's network: ${String(err)}`),
      );
  }, [plane]);
  useEffect(read, [read, moved]);
  return { network, said, setSaid };
}

/** **Open hosts**: each preset in force, host by host, then the project's own. */
export function OpenHostsList({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const { network, said, setSaid } = useNetwork(plane);
  return (
    <div id={ids.id} aria-labelledby={ids.labelledBy}>
      {network === undefined ? (
        said === undefined && <p>Reading this project&apos;s network…</p>
      ) : !network.on ? (
        <p>Chats here run without a sandbox, so they can reach any host.</p>
      ) : network.open.length === 0 ? (
        <p>No host is open: a chat here reaches only what you allow.</p>
      ) : (
        network.open.map((group) => (
          <div key={group.title} className="sandbox-hosts">
            <span className="sandbox-what">{group.title}</span>
            <ul aria-label={group.title}>
              {group.hosts.map((host) => (
                <li key={host}>
                  <code>{host}</code>
                </li>
              ))}
            </ul>
          </div>
        ))
      )}
      {said !== undefined && <Trouble said={said} onDismiss={() => setSaid(undefined)} />}
    </div>
  );
}

/** What a row of a refused lookup says beside its host (#1663): a program's own printed words
 *  named it, so it is shown as text and never offered to allow. */
export const LOOKED_UP = "Looked up by a program, not offered.";

/** One row of a refusals list, as a sentence: what, from which chat, when and how often. */
export function blockedSaid(row: BlockedLately): string {
  const from = row.chat === null ? "" : ` · ${row.chat}`;
  const times = row.times > 1 ? ` · ${row.times} times, last` : "";
  return `${capital(row.said)}${from}${times} · ${when(row.at)}`;
}

/** `said` starting with a capital, as a list row does. */
function capital(said: string): string {
  return said.length > 0 ? said[0].toUpperCase() + said.slice(1) : said;
}

/**
 * The rows of a refusals list (Blocked lately, or a chat's own), each with Allow where the core
 * says one may be kept: **Allow for me on this machine** keeps the host for this project on this
 * machine, and **Allow for everyone in this project** commits it: the registry's words, every
 * level side by side (#1700, `allowLabels.ts`). A row a chat reaches now says so. A refused
 * lookup's host is shown and never has an Allow: a program's printed words named it (#1663).
 */
export function RefusedRows({
  plane,
  rows,
  label,
}: {
  plane: PlaneId;
  rows: readonly BlockedLately[];
  label: string;
}) {
  const [said, setSaid] = useState<string>();
  const [trouble, setTrouble] = useState<string>();
  /** Why an Allow for everyone is off: the project is read-only (#1759). */
  const readOnly = useReadOnlySaid();
  const allow = (host: string, level: GrantLevel) => {
    setSaid(undefined);
    setTrouble(undefined);
    void commands
      .allowBlockedHost(plane, host, level)
      .then((done) => {
        if (done.status === "error") setTrouble(done.error);
        else {
          setSaid(done.data.said);
          allowedOnAList();
        }
      })
      .catch((err: unknown) => setTrouble(`purlis could not allow ${host}: ${String(err)}`))
      // What chats here reach moved, or did not: whatever reads it asks again.
      .finally(sandboxCommandReturned);
  };
  return (
    <>
      <ul className="granted-list" aria-label={label}>
        {rows.map((row) => (
          <li key={`${row.target ?? ""}\u0000${row.looked_up ?? ""}\u0000${row.said}`}>
            {row.target !== null && <code>{row.target}</code>}
            {row.looked_up !== null && <code>{row.looked_up}</code>}
            <span>{blockedSaid(row)}</span>
            {row.reached && <span className="granted-note"> Reached now.</span>}
            {row.looked_up !== null && <span className="granted-note"> {LOOKED_UP}</span>}
            {row.target !== null &&
              row.looked_up === null &&
              row.levels.map((level) => (
                <button
                  key={level}
                  type="button"
                  tabIndex={0}
                  // An Allow for everyone edits the committed file (#1759).
                  disabled={level === "project" && readOnly !== undefined}
                  title={level === "project" ? readOnly : undefined}
                  aria-label={`${ALLOW_SAID[level]}: ${row.target}`}
                  onClick={() => row.target !== null && allow(row.target, level)}
                >
                  {ALLOW_SAID[level]}
                </button>
              ))}
          </li>
        ))}
      </ul>
      {said !== undefined && (
        <Notice cause={`network-allowed:${said}`} at="pane" onDismiss={() => setSaid(undefined)}>
          {said}
        </Notice>
      )}
      {trouble !== undefined && <Trouble said={trouble} onDismiss={() => setTrouble(undefined)} />}
    </>
  );
}

/** **Blocked lately**: what chats here were refused in the last 30 days, newest first. */
export function BlockedLatelyList({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const { network, said, setSaid } = useNetwork(plane);
  return (
    <div id={ids.id} aria-labelledby={ids.labelledBy}>
      {network === undefined ? (
        said === undefined && <p>Reading what was blocked…</p>
      ) : network.blocked.length === 0 ? (
        <p>Nothing was blocked here in the last 30 days.</p>
      ) : (
        <RefusedRows plane={plane} rows={network.blocked} label="Blocked lately" />
      )}
      {said !== undefined && <Trouble said={said} onDismiss={() => setSaid(undefined)} />}
    </div>
  );
}

/** What the port form's scope says for this project on this machine: the Notice's wider Allow. */
const FOR_YOU = "you";

/** The chats a port can be allowed for alone: each chat open here, never a shell. */
function chatsHere(open: readonly OpenChat[]): { value: string; label: string }[] {
  return open
    .filter((one) => one.harness !== null)
    .map((one) => ({ value: String(one.session), label: `only ${one.label ?? one.name}` }));
}

/**
 * **A service on this Mac, added by its port** (#1970): one port, for every chat of this project
 * on this machine or for one open chat alone, which the core judges, audits and keeps through
 * the one function a block's Notice's Allow does (`sandboxing::judged_and_kept`), so whatever
 * the Notice refuses (a range, every port, purlis's own port, a scope past them, a policy lock)
 * is refused here in the same words. The Allowed list beside it shows it once kept.
 */
export function AddPort({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const [typed, setTyped] = useState("");
  const [scope, setScope] = useState(FOR_YOU);
  const [chats, setChats] = useState<{ value: string; label: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string>();
  const [trouble, setTrouble] = useState<string>();
  const moved = useSandboxCommands();
  useEffect(() => {
    let gone = false;
    void commands
      .openedChats(plane)
      .then((done) => {
        if (!gone && done.status === "ok") setChats(chatsHere(done.data ?? []));
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, moved]);
  // A chat that closed is no scope to pick.
  const picked = scope === FOR_YOU || chats.some((one) => one.value === scope) ? scope : FOR_YOU;
  const add = () => {
    const port = typed.trim();
    const level: GrantLevel = picked === FOR_YOU ? "you" : "chat";
    setSaid(undefined);
    setTrouble(undefined);
    setBusy(true);
    void commands
      .allowSandboxPort(plane, port, picked === FOR_YOU ? null : Number(picked), level)
      .then((done) => {
        if (done.status === "error") setTrouble(done.error);
        else {
          setSaid(done.data.said);
          setTyped("");
          allowedOnAList();
        }
      })
      .catch((err: unknown) => setTrouble(`purlis could not allow port ${port}: ${String(err)}`))
      .finally(() => {
        setBusy(false);
        // What chats here reach moved, or did not: whatever reads it asks again.
        sandboxCommandReturned();
      });
  };
  // The window's alone (`WINDOW_ONLY`): a link's page never widens what a chat reaches.
  if (!inTheWindow(commands, "allowSandboxPort"))
    return (
      <p id={ids.id} className="granted-note" aria-labelledby={ids.labelledBy}>
        Add one from purlis's own window on this machine.
      </p>
    );
  return (
    <div aria-labelledby={ids.labelledBy}>
      <Field
        kind="text"
        ids={ids}
        value={typed}
        onChange={setTyped}
        placeholder="50795"
        inputMode="numeric"
        size={7}
        disabled={busy}
      />{" "}
      <label>
        for{" "}
        <Choice
          kind="select"
          ids={{ id: `${ids.id}-for`, label: "Allowed for" }}
          options={[
            { value: FOR_YOU, label: "every chat of this project on this machine" },
            ...chats,
          ]}
          value={picked}
          disabled={busy}
          onValueChange={setScope}
        />
      </label>{" "}
      <button
        type="button"
        className="ui-button"
        tabIndex={0}
        disabled={busy || typed.trim() === ""}
        onClick={add}
      >
        Allow port
      </button>
      {said !== undefined && (
        <Notice cause={`port-allowed:${said}`} at="pane" onDismiss={() => setSaid(undefined)}>
          {said}
        </Notice>
      )}
      {trouble !== undefined && <Trouble said={trouble} onDismiss={() => setTrouble(undefined)} />}
    </div>
  );
}
