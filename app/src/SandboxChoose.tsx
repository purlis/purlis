// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type BlockChoices, type ChatBlocked } from "./bindings";
import { asksMoved } from "./asks";
import { inTheWindow } from "./personaHostsAllow";
import { inertly } from "./sandboxBlocks";
import { Choice, Field, SettingRow } from "./settings/components";
import { GrantsMark } from "./primitives/Grants";

/** The pick that opens the box a path is typed in: no socket's path, which starts at `/`. */
const ANOTHER = "another";

/** Whether this page can carry the person's choice: the window's alone (`WINDOW_ONLY`). */
export const mayChoose = (): boolean =>
  inTheWindow(commands, "sandboxBlockChoices") && inTheWindow(commands, "chooseSandboxBlockTarget");

/** The words for what `block` lets the person choose. */
export function chooseSaid(block: ChatBlocked): string {
  return block.choose === "service" ? "Choose the service…" : "Choose the socket…";
}

/**
 * **The person names what a sandbox block did not** (#1884; ADR 0093 as amended 2026-10-13):
 * the form a block's Notice opens under its line, beside Start without the sandbox.
 *
 * It lists what **the app itself** found on this machine (`sandbox_block_choices`): the sockets
 * of the services purlis recognises, each a socket a grant may name now, or the services purlis
 * can allow. For a socket it also takes a path the person types. **Nothing a chat wrote is
 * here**: the block carries no target to fill the box with, the list is the app's own, in the
 * core's order, and the box starts empty.
 *
 * **Use this socket** sends the choice (`choose_sandbox_block_target`), which grants nothing.
 * The core judges it as it judges a socket the sandbox named, and on a yes tells the window the
 * block again, now naming it: this Notice gives way to the named one, with what allowing it
 * means and Allow once, for this chat and always. A choice it refuses is said here, with why,
 * and the person may choose again.
 */
export function SandboxChoose({ block, id }: { block: ChatBlocked; id: string }) {
  const what = block.choose === "service" ? "service" : "socket";
  const [listed, setListed] = useState<BlockChoices>();
  const [picked, setPicked] = useState<string>();
  const [typed, setTyped] = useState("");
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const shown = { operation: block.operation, kind: block.kind, what, target: "" } as const;
  const { plane, session, operation, kind } = block;

  useEffect(() => {
    let gone = false;
    void commands
      .sandboxBlockChoices(plane, session, { operation, kind, what, target: "" })
      .then((found) => {
        if (gone) return;
        if (found.status === "error") {
          setSaid(found.error);
          return;
        }
        setListed(found.data);
        // Nothing found: the box to type one in is all there is, so it is open already.
        if (found.data.choices.length === 0 && found.data.typed) setPicked(ANOTHER);
      })
      .catch((err: unknown) => {
        if (!gone) setSaid(`purlis could not look for what to choose: ${String(err)}`);
      });
    return () => {
      gone = true;
    };
  }, [plane, session, operation, kind, what]);

  const target = picked === ANOTHER ? typed.trim() : (picked ?? "");
  const choose = () => {
    if (busy || target === "") return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .chooseSandboxBlockTarget(plane, session, shown, target)
      .then((chosen) => {
        if (chosen.status === "error") setSaid(chosen.error);
        // The core tells the window the block again, naming it; its ask is listed from now on.
        else asksMoved(plane);
      })
      .catch((err: unknown) => setSaid(`purlis could not take that choice: ${String(err)}`))
      .finally(() => setBusy(false));
  };

  const options = [
    ...(listed?.choices ?? []).map((one) => ({
      value: one.target,
      label: inertly(one.target),
      says: `${one.named}. ${one.says}`,
    })),
    ...(listed?.typed === true
      ? [{ value: ANOTHER, label: "Another socket", says: "Type its whole path." }]
      : []),
  ];
  const none = listed !== undefined && listed.choices.length === 0;

  return (
    <div className="block-allow" id={id}>
      {listed === undefined ? (
        said === undefined && <p>Looking for what purlis can allow on this machine…</p>
      ) : (
        <>
          <SettingRow
            label={what === "socket" ? "Socket to allow" : "Service to allow"}
            help={
              what === "socket"
                ? none
                  ? "purlis found no socket it recognises on this machine. Nothing is allowed until you press an Allow."
                  : "purlis found these on this machine. Nothing is allowed until you press an Allow."
                : "The services purlis can allow. Nothing is allowed until you press an Allow."
            }
            control={(ids) => (
              <Choice
                kind="radio"
                ids={ids}
                options={options}
                value={picked}
                onValueChange={setPicked}
                disabled={busy}
              />
            )}
          />
          {picked === ANOTHER && (
            <SettingRow
              label="Socket path"
              help="The whole path of a socket on this machine, from /."
              control={(ids) => (
                <Field
                  kind="text"
                  ids={ids}
                  value={typed}
                  onChange={setTyped}
                  placeholder="/var/run/docker.sock"
                />
              )}
            />
          )}
          <div className="block-allow-actions ui-button-row">
            <button
              type="button"
              // It grants what is chosen: drawn as every grant is (ADR 0097 §6).
              className="ui-button ui-button-sm ui-button-grants"
              tabIndex={0}
              disabled={busy || target === ""}
              onClick={choose}
            >
              <GrantsMark />
              {what === "socket" ? "Use this socket" : "Use this service"}
            </button>
          </div>
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}
