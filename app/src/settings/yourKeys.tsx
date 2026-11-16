// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useSyncExternalStore } from "react";
import { commands, type YourKeyRow } from "../bindings";
import { KeyForm } from "../KeyForm";
import { Button } from "../ui/button/Button";
import { AlertDialog } from "../ui/dialog/Dialog";
import { Reserved } from "../ui/dialog/Reserved";
import {
  ControlBody,
  ControlCell,
  ControlColumn,
  ControlHead,
  ControlRow,
  ControlRowHeading,
  ControlTable,
} from "../ui/table/ControlTable";
import { windowCopy } from "../windowCopy";
import { YOUR_KEYS_ALSO } from "./catalogue";
import type { LiveSetting, SettingsGroup } from "./groups";
import { REPLACE_THE_KEY, yourKeyLink } from "./links";

/** Its one setting, the list, which a link names to land on a key's row (`yourKeyLink`). */
const YOUR_KEYS_LIST = yourKeyLink("").setting ?? "";

/**
 * **You › Your keys** (T1a-11, R-171, R-578): the model keys this machine keeps, each a
 * keychain value only purlis reads. Built as You › Profiles is (V21): core reads the list and
 * answers it through the IPC (`your_keys`), with its reading and trouble states, and every act
 * is a core command, the window's alone. Nothing here is the window's own state of a key: the
 * product, the origins its mark binds it to, its approval setting, what the check at save found,
 * the profiles, chats and pools using it are all core's.
 *
 * - **Add a key…** opens the one key form (`KeyForm`) in place under the list's head, aimed at a
 *   new entry; **Replace the key…** opens it under the row, aimed at that entry, so a rotated key
 *   keeps its id, its mark and its pools. Never a dialog (X3).
 * - **Remove…** always asks, with W5's count of the chats using the key, and has no undo (U19).
 *   After it the keyboard goes to the next row, or to *Add a key…* (G-10).
 * - **A row's states**: W55 (its item is gone: *Remove…* only), W6 (the keychain locked:
 *   *Unlock*), W60 (changed outside purlis: *Mark again*), W7 (*not checked yet*: *Try again*)
 *   and W46 (*refused at save*). There is no *Unmark* (U8).
 * - **Each pool the key is sent to** has *Stop sending*, which asks with W123's count first.
 */
export function yourKeysGroup(): SettingsGroup {
  return {
    id: "you.models",
    label: "Your keys",
    help: "The model keys this machine keeps for your chats, each in your system keychain. Only purlis reads a key, and only to send it to the provider it is for.",
    also: YOUR_KEYS_ALSO,
    settings: [listed],
  };
}

/** An act running on a row, so its button is busy and the others wait. */
type Busy = "remove" | "mark" | "unlock" | "check" | `stop:${string}`;

/** A question the page is asking: Remove's (W5) or Stop sending's (W123), with core's count. */
type Asking =
  | { kind: "remove"; row: YourKeyRow; count: number }
  | { kind: "stop"; row: YourKeyRow; pool: string; count: number };

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; rows: YourKeyRow[] };

type Held = {
  read: Read;
  /** *Try again* on the trouble state, while core reads. */
  rereading: boolean;
  /** The act running on each row, by its id. */
  busy: Record<string, Busy>;
  /** The line under a row an act that failed left, by its id (W84, W114, W128). */
  failed: Record<string, string>;
  /** The line under a pool's line *Stop sending* left, by id and pool (W129). */
  poolFailed: Record<string, string>;
  /** The question up, if any. */
  asking: Asking | undefined;
  /** Where the key form is open: under the head (`new`) or under a row (its id). */
  form: "new" | string | undefined;
  /** Where the keyboard goes once the list no longer holds the key removed (G-10): the row now
   *  at its place, or Add a key…. */
  focusAfter: { at: number; gone: string } | undefined;
};

const START: Held = {
  read: { state: "reading" },
  rereading: false,
  busy: {},
  failed: {},
  poolFailed: {},
  asking: undefined,
  form: undefined,
  focusAfter: undefined,
};
let held: Held = START;
const listeners = new Set<() => void>();
let reading = 0;

function set(change: (was: Held) => Held) {
  held = change(held);
  for (const one of listeners) one();
}

type Done<T> = { status: "ok"; data: T } | { status: "error"; error: string };

const answered = <T,>(asked: Promise<Done<T>>): Promise<Done<T>> =>
  asked.catch((err: unknown) => ({ status: "error" as const, error: String(err) }));

async function reread() {
  const mine = ++reading;
  const said = await answered(commands.yourKeys());
  if (mine !== reading) return;
  set((was) => ({
    ...was,
    rereading: false,
    read:
      said.status === "error"
        ? { state: "trouble", trouble: said.error }
        : Array.isArray(said.data)
          ? { state: "read", rows: said.data }
          : { state: "trouble", trouble: "purlis did not say which keys this machine keeps" },
  }));
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = START;
    void reread();
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

const without = <T,>(record: Record<string, T>, key: string): Record<string, T> =>
  Object.fromEntries(Object.entries(record).filter(([one]) => one !== key));

const poolKey = (id: string, pool: string) => `${id}\u0000${pool}`;

/** Runs `act` on row `id` as `busy`; on failure says `fails(error)` under it; reads again. */
async function onRow(
  id: string,
  busy: Busy,
  act: () => Promise<Done<unknown>>,
  fails: (error: string) => string,
  done?: (data: unknown) => void,
) {
  set((was) => ({ ...was, busy: { ...was.busy, [id]: busy }, failed: without(was.failed, id) }));
  const said = await answered(act());
  set((was) => ({
    ...was,
    busy: without(was.busy, id),
    failed: said.status === "error" ? { ...was.failed, [id]: fails(said.error) } : was.failed,
  }));
  if (said.status === "ok") done?.(said.data);
  await reread();
}

async function askRemove(row: YourKeyRow) {
  set((was) => ({ ...was, failed: without(was.failed, row.id) }));
  const said = await answered(commands.yourKeyChats(row.id));
  if (said.status === "error") {
    set((was) => ({
      ...was,
      failed: { ...was.failed, [row.id]: windowCopy.W114({ error: said.error }) },
    }));
    return;
  }
  set((was) => ({ ...was, asking: { kind: "remove", row, count: said.data } }));
}

function remove(row: YourKeyRow) {
  const rows = held.read.state === "read" ? held.read.rows : [];
  const at = rows.findIndex((one) => one.id === row.id);
  set((was) => ({ ...was, asking: undefined }));
  void onRow(
    row.id,
    "remove",
    () => commands.yourKeyRemove(row.id),
    (error) => windowCopy.W114({ error }),
    () => set((was) => ({ ...was, focusAfter: { at, gone: row.id } })),
  );
}

async function askStop(row: YourKeyRow, pool: string) {
  set((was) => ({ ...was, poolFailed: without(was.poolFailed, poolKey(row.id, pool)) }));
  const said = await answered(commands.yourKeyPoolChats(row.id, pool));
  if (said.status === "error") {
    set((was) => ({
      ...was,
      poolFailed: {
        ...was.poolFailed,
        [poolKey(row.id, pool)]: windowCopy.W129({ pool, error: said.error }),
      },
    }));
    return;
  }
  set((was) => ({ ...was, asking: { kind: "stop", row, pool, count: said.data } }));
}

async function stop(row: YourKeyRow, pool: string) {
  const key = poolKey(row.id, pool);
  set((was) => ({
    ...was,
    asking: undefined,
    busy: { ...was.busy, [row.id]: `stop:${pool}` },
    poolFailed: without(was.poolFailed, key),
  }));
  const said = await answered(commands.yourKeyStopSending(row.id, pool));
  set((was) => ({
    ...was,
    busy: without(was.busy, row.id),
    poolFailed:
      said.status === "error"
        ? { ...was.poolFailed, [key]: windowCopy.W129({ pool, error: said.error }) }
        : was.poolFailed,
  }));
  await reread();
}

function unlock(row: YourKeyRow) {
  void onRow(
    row.id,
    "unlock",
    () => commands.yourKeyUnlock(row.id),
    () => windowCopy.W84,
  );
}

/** A row's button: `spoken` names the key it acts on. `tabIndex` as Profiles' (#190). */
function RowButton({
  label,
  spoken,
  busyLabel,
  busy,
  disabled,
  onPress,
}: {
  label: string;
  spoken: string;
  busyLabel?: string;
  busy?: boolean;
  disabled: boolean;
  onPress: () => void;
}) {
  return (
    <Button
      aria-label={spoken}
      isDisabled={disabled || busy}
      onPress={onPress}
      className="ui-setting-reset"
    >
      {busyLabel === undefined ? (
        label
      ) : (
        <Reserved texts={[label, busyLabel]}>{busy ? busyLabel : label}</Reserved>
      )}
    </Button>
  );
}

/** What the row says of its users. */
function usedBy(row: YourKeyRow): string {
  const profiles =
    row.profiles.length === 0 ? "No profile uses it." : `Used by ${row.profiles.join(", ")}.`;
  const chats = row.chats.length === 0 ? "" : ` Chats: ${row.chats.join(", ")}.`;
  return profiles + chats;
}

const listed: LiveSetting = {
  id: YOUR_KEYS_LIST,
  label: "Keys on this machine",
  help: "Replace the key… writes a new value under the same key, so its profiles, chats and pools keep it. Remove… asks first and says how many chats use the key. There is no undo.",
  writes: "this machine",
  useControl: function useYourKeys() {
    const now = useSyncExternalStore(subscribe, () => held);
    const holder = useRef<HTMLDivElement>(null);
    // **After Remove** (G-10): the keyboard goes to the next row's first button, or to Add a key….
    useEffect(() => {
      const after = now.focusAfter;
      if (after === undefined || now.read.state !== "read") return;
      if (now.read.rows.some((one) => one.id === after.gone)) return;
      const root = holder.current;
      if (root === null) return;
      const rows = [...root.querySelectorAll<HTMLElement>("[data-key-row]")];
      const next = rows[Math.min(after.at, rows.length - 1)];
      const target =
        next?.querySelector<HTMLElement>("button:not([disabled])") ??
        root.querySelector<HTMLElement>("[data-add-a-key]");
      target?.focus();
      set((was) => ({ ...was, focusAfter: undefined }));
    }, [now.focusAfter, now.read]);
    const anyBusy = Object.keys(now.busy).length > 0;
    const addAKey = (
      <Button
        data-add-a-key
        isDisabled={now.form === "new"}
        onPress={() => set((was) => ({ ...was, form: "new" }))}
      >
        Add a key…
      </Button>
    );
    const closeForm = () => set((was) => ({ ...was, form: undefined }));
    const saved = () => {
      closeForm();
      void reread();
    };
    return {
      grouped: true,
      control: (ids) => (
        <div
          ref={holder}
          className="your-keys"
          role="group"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
        >
          {now.read.state === "reading" && (
            <p className="ui-setting-help ui-text-secondary">Reading your keys…</p>
          )}
          {now.read.state === "trouble" && (
            <div className="your-keys-trouble">
              <p className="trouble ui-setting-help" role="alert">
                {windowCopy.W115({ error: now.read.trouble })}
              </p>
              <Button
                isDisabled={now.rereading}
                onPress={() => {
                  set((was) => ({ ...was, rereading: true }));
                  void reread();
                }}
              >
                <Reserved texts={["Try again", "Reading…"]}>
                  {now.rereading ? "Reading…" : "Try again"}
                </Reserved>
              </Button>
            </div>
          )}
          {now.read.state === "read" && (
            <>
              <div className="your-keys-head">
                {now.read.rows.length === 0 && (
                  <p className="ui-setting-help ui-text-secondary">{windowCopy.W4}</p>
                )}
                {addAKey}
              </div>
              {now.form === "new" && (
                <KeyForm target={{ kind: "new-entry" }} onSaved={saved} onCancel={closeForm} />
              )}
              {now.read.rows.length > 0 && (
                <ControlTable
                  className="your-keys-table ui-record-table"
                  fixed
                  stacks
                  aria-label="Your keys"
                  aria-describedby={ids.describedBy}
                >
                  <ControlHead>
                    <ControlColumn>Key</ControlColumn>
                    <ControlColumn>Used by</ControlColumn>
                    <ControlColumn actions hideName width="10rem">
                      Actions
                    </ControlColumn>
                  </ControlHead>
                  <ControlBody>
                    {now.read.rows.flatMap((row) => {
                      const busy = now.busy[row.id];
                      const off = anyBusy || now.asking !== undefined;
                      const drawn = [
                        <ControlRow key={row.id} data-key-row={row.id}>
                          <ControlRowHeading>
                            <span className="ui-record-lines">
                              <span className="your-keys-product">{row.product}</span>
                              {row.origins.length > 0 && (
                                <span className="ui-setting-help ui-text-secondary">
                                  Sent to {row.origins.join(", ")}
                                </span>
                              )}
                              <span className="ui-setting-help ui-text-secondary">
                                Asks before each use: {row.approval}
                              </span>
                              {row.state === "missing" && (
                                <span className="ui-setting-help">{windowCopy.W55}</span>
                              )}
                              {row.state === "locked" && (
                                <span className="ui-setting-help">{windowCopy.W6}</span>
                              )}
                              {row.state === "unmarked" && (
                                <span className="ui-setting-help">{windowCopy.W60}</span>
                              )}
                              {row.state !== "missing" && row.checked === "not checked yet" && (
                                <span className="ui-setting-help">{windowCopy.W7}</span>
                              )}
                              {row.state !== "missing" && row.checked === "refused at save" && (
                                <span className="ui-setting-help">{windowCopy.W46}</span>
                              )}
                              {now.failed[row.id] !== undefined && (
                                <span className="trouble ui-setting-help" role="alert">
                                  {now.failed[row.id]}
                                </span>
                              )}
                            </span>
                          </ControlRowHeading>
                          <ControlCell heading="Used by">
                            <span className="ui-record-lines">
                              <span className="ui-setting-help ui-text-secondary">
                                {usedBy(row)}
                              </span>
                              {row.pools.map((pool) => (
                                <span
                                  key={pool}
                                  className="ui-record-lines"
                                  aria-label={`Pool ${pool}`}
                                  role="group"
                                >
                                  <span className="ui-setting-help ui-text-secondary">
                                    Sent to pool {pool}
                                  </span>
                                  <RowButton
                                    label={windowCopy.W125}
                                    spoken={`${windowCopy.W125} to ${pool}`}
                                    busyLabel="Stopping…"
                                    busy={busy === `stop:${pool}`}
                                    disabled={off}
                                    onPress={() => void askStop(row, pool)}
                                  />
                                  {now.poolFailed[poolKey(row.id, pool)] !== undefined && (
                                    <span className="trouble ui-setting-help" role="alert">
                                      {now.poolFailed[poolKey(row.id, pool)]}
                                    </span>
                                  )}
                                </span>
                              ))}
                            </span>
                          </ControlCell>
                          <ControlCell actions>
                            <span className="your-keys-actions">
                              {row.state === "locked" && (
                                <RowButton
                                  label="Unlock"
                                  spoken={`Unlock the key for ${row.product}`}
                                  busyLabel="Unlocking…"
                                  busy={busy === "unlock"}
                                  disabled={off}
                                  onPress={() => unlock(row)}
                                />
                              )}
                              {row.state === "unmarked" && (
                                <RowButton
                                  label={windowCopy.W126}
                                  spoken={`${windowCopy.W126}: the key for ${row.product}`}
                                  busyLabel="Marking…"
                                  busy={busy === "mark"}
                                  disabled={off}
                                  onPress={() =>
                                    void onRow(
                                      row.id,
                                      "mark",
                                      () => commands.yourKeyMarkAgain(row.id),
                                      (error) => windowCopy.W128({ error }),
                                    )
                                  }
                                />
                              )}
                              {row.state !== "missing" && row.checked === "not checked yet" && (
                                <RowButton
                                  label="Try again"
                                  spoken={`Try again: check the key for ${row.product}`}
                                  busyLabel="Checking…"
                                  busy={busy === "check"}
                                  disabled={off}
                                  onPress={() =>
                                    void onRow(
                                      row.id,
                                      "check",
                                      () => commands.yourKeyCheckAgain(row.id),
                                      (error) => error,
                                    )
                                  }
                                />
                              )}
                              {row.state !== "missing" && (
                                <RowButton
                                  label={REPLACE_THE_KEY}
                                  spoken={`${REPLACE_THE_KEY} for ${row.product}`}
                                  disabled={off || now.form === row.id}
                                  onPress={() => set((was) => ({ ...was, form: row.id }))}
                                />
                              )}
                              <RowButton
                                label="Remove…"
                                spoken={`Remove the key for ${row.product}`}
                                busyLabel="Removing…"
                                busy={busy === "remove"}
                                disabled={off}
                                onPress={() => void askRemove(row)}
                              />
                            </span>
                          </ControlCell>
                        </ControlRow>,
                      ];
                      if (now.form === row.id)
                        drawn.push(
                          <ControlRow key={`${row.id}-form`}>
                            <ControlCell colSpan={3}>
                              <KeyForm
                                target={{ kind: "this-entry", id: row.id }}
                                onSaved={saved}
                                onCancel={closeForm}
                              />
                            </ControlCell>
                          </ControlRow>,
                        );
                      return drawn;
                    })}
                  </ControlBody>
                </ControlTable>
              )}
            </>
          )}
          {now.asking?.kind === "remove" && (
            <AlertDialog
              onClose={() => set((was) => ({ ...was, asking: undefined }))}
              title={`Remove the key for ${now.asking.row.product}?`}
              description={
                <p className="came-back ui-text-secondary">
                  {windowCopy.W5({ count: now.asking.count })} There is no undo.
                </p>
              }
              main={{
                children: "Remove",
                variant: "danger",
                onPress: () => {
                  if (now.asking?.kind === "remove") remove(now.asking.row);
                },
              }}
            />
          )}
          {now.asking?.kind === "stop" && (
            <AlertDialog
              onClose={() => set((was) => ({ ...was, asking: undefined }))}
              title={`${windowCopy.W125}?`}
              description={
                <p className="came-back ui-text-secondary">
                  {windowCopy.W123({ count: now.asking.count, pool: now.asking.pool })}
                </p>
              }
              main={{
                children: windowCopy.W125,
                variant: "danger",
                onPress: () => {
                  if (now.asking?.kind === "stop") void stop(now.asking.row, now.asking.pool);
                },
              }}
            />
          )}
        </div>
      ),
    };
  },
};
