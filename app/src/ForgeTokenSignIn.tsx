// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type ForgeAccountRow, type ForgeTokenHelp } from "./bindings";
import { Choice, Field, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";
import { TextField } from "./ui/field/Field";

/** The forges a token signs in to, and the host each defaults to. */
export const FORGES = [
  {
    value: "github",
    label: "GitHub",
    host: "github.com",
    says: "github.com or a GitHub Enterprise server",
  },
  {
    value: "gitlab",
    label: "GitLab",
    host: "gitlab.com",
    says: "gitlab.com or a self-managed GitLab",
  },
] as const;

export type ForgeWord = (typeof FORGES)[number]["value"];

/**
 * **Sign in with a token** (#1814, ADR 0077 §4): a forge, its host, and a token pasted for it.
 * One component, used by Settings › Accounts and by the welcome's "Sign in" (#1812).
 *
 * - **The scopes and the page that makes a token** are the core's (`forge_token_help`), asked
 *   again as the host changes: the dialog lists the exact scopes and opens the forge's own token
 *   page in the browser.
 * - **The token is handed over once and is never on the page.** The box is uncontrolled, read
 *   from the element at the press and emptied there, as a vault's token box is. Nothing in React
 *   state, no attribute and no answer carries it.
 * - **Every refusal is the core's sentence**, which names a missing scope or what is wrong with
 *   the host, and never repeats the token. The person can fix the box and press Sign in again.
 */
export function ForgeTokenSignIn({
  forge: initial = "github",
  onSignedIn,
  onCancel,
}: {
  /** The forge chosen when it opens. */
  forge?: ForgeWord;
  /** The account the token signed in to, as Settings lists it. */
  onSignedIn: (account: ForgeAccountRow) => void;
  onCancel: () => void;
}) {
  const [forge, setForge] = useState<ForgeWord>(initial);
  const [host, setHost] = useState<string>(FORGES.find((f) => f.value === initial)?.host ?? "");
  /** Whether the host is still the forge's default, so choosing the other forge moves it. */
  const [hostTyped, setHostTyped] = useState(false);
  const [help, setHelp] = useState<ForgeTokenHelp | undefined>();
  const [hostTrouble, setHostTrouble] = useState<string | undefined>();
  const [trouble, setTrouble] = useState<string | undefined>();
  const [checking, setChecking] = useState(false);
  const box = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let current = true;
    void commands
      .forgeTokenHelp(forge, host)
      .then((said) => {
        if (!current) return;
        if (said.status === "ok") {
          setHelp(said.data);
          setHostTrouble(undefined);
        } else {
          setHelp(undefined);
          setHostTrouble(said.error);
        }
      })
      .catch((err: unknown) => {
        if (current) setHostTrouble(String(err));
      });
    return () => {
      current = false;
    };
  }, [forge, host]);

  const chooseForge = (to: string) => {
    const next = FORGES.find((f) => f.value === to);
    if (!next) return;
    setForge(next.value);
    if (!hostTyped) setHost(next.host);
  };

  /** Read the token from the box once, empty the box, and hand it to the core. */
  const signIn = async () => {
    if (checking) return;
    const token = box.current?.value ?? "";
    if (box.current) box.current.value = "";
    setTrouble(undefined);
    setChecking(true);
    const said = await commands
      .forgeSignInWithToken(forge, host, token)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setChecking(false);
    if (said.status === "ok") onSignedIn(said.data);
    else {
      setTrouble(said.error);
      box.current?.focus();
    }
  };

  return (
    <Dialog
      onClose={() => {
        // Not while the forge is being asked: the answer would land where nobody is looking.
        if (!checking) onCancel();
      }}
      title="Sign in with a token"
      subtitle="for this machine; the token is kept in your system keychain"
      cancel={{ isDisabled: checking, onPress: onCancel }}
      main={{
        children: checking ? "Checking…" : "Sign in",
        reserve: ["Sign in", "Checking…"],
        isDisabled: checking || help === undefined,
        onPress: () => void signIn(),
      }}
    >
      <SettingRow
        label="Forge"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={FORGES.map((one) => ({
              value: one.value,
              label: one.label,
              says: one.says,
            }))}
            value={forge}
            onValueChange={chooseForge}
            disabled={checking}
          />
        )}
      />

      <SettingRow
        label="Host"
        help="The forge's address. Change it for a GitHub Enterprise server or a self-managed GitLab."
        error={hostTrouble === undefined ? undefined : [hostTrouble]}
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={host}
            placeholder="git.example.com"
            disabled={checking}
            onChange={(to) => {
              setHost(to);
              setHostTyped(true);
            }}
          />
        )}
      />

      {help !== undefined && (
        <div className="ui-setting-help" aria-live="polite">
          <p>
            Make a token on {help.host} with {help.scopes.length === 1 ? "the scope" : "the scopes"}{" "}
            {help.scopes.map((scope, i) => (
              <span key={scope}>
                {i > 0 && (i === help.scopes.length - 1 ? " and " : ", ")}
                <code>{scope}</code>
              </span>
            ))}
            .{" "}
            <Button onPress={() => void openUrl(help.create_at)} className="ui-setting-reset">
              Make a token on {help.host}
            </Button>
          </p>
        </div>
      )}

      <SettingRow
        label="Token"
        help="It goes to purlis once, is checked with the forge, and is kept in your system keychain. It never reaches a chat."
        control={(ids) => (
          <TextField
            inputRef={box}
            id={ids.id}
            className="ui-setting-box"
            type="password"
            placeholder="Paste the token"
            isDisabled={checking}
            aria-labelledby={ids.labelledBy}
            aria-describedby={ids.describedBy}
          />
        )}
      />

      {trouble !== undefined && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}
