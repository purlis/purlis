// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * What one sign-in says and asks at each state, the vendor's flow as the window drives it on
 * the runner. Shared by the three variants: they disagree about where this sits and how a
 * person gets to it, not about what the vendor asks.
 */
import { useState } from "react";
import { Badge } from "../../ui/badge/Badge";
import { Button } from "../../ui/button/Button";
import { Card } from "../../ui/card/Card";
import { TextField } from "../../ui/field/Field";
import { FAKE_TERMINAL_LINES, PROTOTYPE_POOLS, type Focus, type SignInState } from "./data";

/** The note a fresh cluster claim's sign-in carries. */
export function FreshClaimNote({ pool }: { pool: string }) {
  return (
    <p className="ui-setting-help proto-rsi-note">
      This sign-in lasts only for this claim. When {pool}&apos;s claim is released, the next runner
      asks you to sign in again.
    </p>
  );
}

/** "Open in terminal": the small way out for someone who wants the runner's own shell. */
function OpenInTerminal() {
  return (
    <Button
      size="sm"
      variant="link"
      tooltip="Opens a shell tab on the runner with the harness's own sign-in typed in"
    >
      Open in terminal
    </Button>
  );
}

/** The static fake of the runner's live terminal, drawn inline when purlis can't follow. */
export function FakeTerminal({ pool }: { pool: string }) {
  return (
    <pre className="proto-rsi-terminal" aria-label={`The runner's terminal on ${pool}`}>
      {FAKE_TERMINAL_LINES.join("\n")}
      <span className="proto-rsi-caret" aria-hidden="true">
        ▍
      </span>
    </pre>
  );
}

/**
 * The sign-in's body at `state`, for `focus`. "choose" is each variant's own, so it draws
 * nothing here.
 */
export function SignInStep({ state, focus }: { state: SignInState; focus: Focus }) {
  const [code, setCode] = useState("");
  const { pool, profile } = focus;
  const fresh = PROTOTYPE_POOLS.find((one) => one.name === pool)?.freshClaim === true;
  const who = `${profile.name} (${profile.harnessSaid})`;
  switch (state) {
    case "choose":
      return null;
    case "opening":
      return (
        <div className="proto-rsi-step">
          <p className="pending" role="status" aria-busy="true">
            Starting {profile.harnessSaid}&apos;s sign-in on {pool}&apos;s runner, then opening its
            page in your browser…
          </p>
          {fresh && <FreshClaimNote pool={pool} />}
          <OpenInTerminal />
        </div>
      );
    case "paste":
    case "fresh-claim":
      return (
        <div className="proto-rsi-step">
          {(fresh || state === "fresh-claim") && <FreshClaimNote pool={pool} />}
          <p>
            Sign in on the page that opened in your browser. It shows a code at the end: paste it
            here.
          </p>
          <div className="proto-rsi-paste">
            <TextField
              label="Code from the browser"
              value={code}
              onChange={setCode}
              autoComplete="off"
              spellCheck={false}
              placeholder="Paste the code"
            />
            <Button variant="primary" isDisabled={code.trim() === ""}>
              Sign in
            </Button>
          </div>
          <p className="ui-setting-help">
            The code signs in {who} on {pool}&apos;s runner only. Nothing of this Mac&apos;s sign-in
            goes there.
          </p>
          <div className="proto-rsi-actions">
            <Button size="sm" variant="ghost">
              Open the page again
            </Button>
            <OpenInTerminal />
          </div>
        </div>
      );
    case "approve":
      return (
        <div className="proto-rsi-step">
          <p>
            Approve purlis on the page that opened in your browser. The sign-in finishes here on its
            own.
          </p>
          <p className="pending" role="status" aria-busy="true">
            Waiting for you to approve it in the browser…
          </p>
          <div className="proto-rsi-actions">
            <Button size="sm" variant="ghost">
              Open the page again
            </Button>
            <OpenInTerminal />
          </div>
        </div>
      );
    case "device":
      return (
        <div className="proto-rsi-step">
          <p>
            Enter this code on GitHub&apos;s page, then approve it there. The code is for this
            sign-in only.
          </p>
          <Card as="p" variant="ask" className="forge-device-code">
            <strong>
              <code aria-label="The code to enter on GitHub">WDJB-MJHT</code>
            </strong>
          </Card>
          <div className="proto-rsi-actions">
            <Button variant="primary">Open GitHub in your browser</Button>
            <Button>Copy code</Button>
            <OpenInTerminal />
          </div>
          <p className="pending" role="status" aria-busy="true">
            Waiting for you to approve it on GitHub…
          </p>
        </div>
      );
    case "signed-in":
      return (
        <div className="proto-rsi-step">
          <p role="status">
            <Badge variant="ok">Signed in</Badge> {who} is signed in on {pool}. The runner keeps it
            across updates.
          </p>
        </div>
      );
    case "terminal":
      return (
        <div className="proto-rsi-step">
          <p>
            purlis could not follow this sign-in, so here is the runner&apos;s own terminal. Finish
            it here; purlis checks again when it ends.
          </p>
          <FakeTerminal pool={pool} />
          <div className="proto-rsi-actions">
            <Button size="sm" variant="ghost">
              Check again
            </Button>
            <OpenInTerminal />
          </div>
        </div>
      );
    case "failed":
      return (
        <div className="proto-rsi-step">
          <p className="trouble said-in-full" role="alert">
            {who} did not sign in on {pool}: the browser&apos;s answer could not reach the runner,
            because another program on this Mac uses port 1455. Quit it, then try again.
          </p>
          <div className="proto-rsi-actions">
            <Button variant="primary">Try again</Button>
            <OpenInTerminal />
          </div>
        </div>
      );
  }
}
