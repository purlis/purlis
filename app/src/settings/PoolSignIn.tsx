// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, type FormEvent } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type PoolSignInStep } from "../bindings";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { TextField } from "../ui/field/Field";
import { SettingActions } from "./components";

/** Where a sign-in is, as the strip draws it: starting, one of the core's steps, or refused. */
export type StripStep = { at: "starting" } | PoolSignInStep | { at: "not-started"; why: string };

/**
 * **The step the strip is on, in words** (spec #4023 story 12): "Step 2 of 3: …". A device code's
 * sign-in (`device`) opens no page as it starts.
 */
export function stepSaid(step: StripStep, device = false): string | null {
  switch (step.at) {
    case "starting":
      if (device) return "Step 1 of 3: Starting the sign-in on the runner";
      return "Step 1 of 3: Opening the sign-in page in your browser";
    case "open":
      return "Step 1 of 3: Opening the sign-in page in your browser";
    case "device":
      return "Step 2 of 3: Enter the code on the page";
    case "code":
    case "refused":
      return "Step 2 of 3: Paste the code the page shows";
    case "checking":
      return "Step 3 of 3: Checking it is signed in on the runner";
    default:
      return null;
  }
}

/**
 * **Signs `profile` in on pool `pool`'s runner, in place** (#4042, spec #4023, prototype Variant
 * A): the strip a profile's line on a pool row opens into. purlis runs the harness's own login on
 * the runner (`pool_sign_in_start`), opens its page in the browser here, takes the code the page
 * shows in one field and passes it back, and says Signed in once the harness's own status check
 * on the runner agrees. opencode's Copilot sign-in (#4045) shows its device code first, in the
 * window's device-code pattern (as `ForgeDeviceSignIn` shows it): the code, then the page on the
 * vendor's own https host, opened when the person asks. Nothing of this Mac's sign-in goes to the
 * runner (ADR 0095).
 *
 * Each step is said by the core (`pool_sign_in_next`); the strip draws it, says which step it is
 * on, moves the keyboard to the code field when it appears, and says each step change to a
 * screen reader. Cancel, or closing the strip, stops driving the login (the login itself on the
 * runner: #4043).
 */
export function PoolSignIn({
  pool,
  profile,
  harness,
  provider,
  onEnd,
  onTerminal,
  onSignedIn,
}: {
  pool: string;
  profile: string;
  /** The harness as people call it: `Claude Code`. */
  harness: string;
  /** The plan it signs in to, for a harness that serves several. */
  provider?: string;
  /** The strip is closed; `signedIn` when it ended signed in. */
  onEnd: (signedIn: boolean) => void;
  /** "Open in terminal", where the window can open one. */
  onTerminal?: () => void;
  /** Told once the runner says it is signed in, before anything is pressed (#3477). */
  onSignedIn?: () => void;
}) {
  const [step, setStep] = useState<StripStep>({ at: "starting" });
  const [page, setPage] = useState<string>();
  const [code, setCode] = useState("");
  const [pasteRefused, setPasteRefused] = useState<string>();
  const [attempt, setAttempt] = useState(0);
  const flow = useRef<number>(undefined);
  const signedIn = useRef(onSignedIn);
  useEffect(() => {
    signedIn.current = onSignedIn;
  }, [onSignedIn]);

  // One try: start it, then ask for each step until the last. Leaving the strip, or trying again,
  // cancels it.
  useEffect(() => {
    let left = false;
    const said = (err: unknown) => ({ status: "error" as const, error: String(err) });
    const follow = async (which: number) => {
      for (;;) {
        const next = await commands.poolSignInNext(which).catch(said);
        if (left) return;
        if (next.status === "error") {
          setStep({ at: "failed", why: next.error });
          return;
        }
        if (next.data === null) continue;
        const now = next.data;
        if (now.at === "open") {
          setPage(now.url);
          void openUrl(now.url).catch(() => undefined);
        }
        setStep(now);
        if (now.at === "signed-in") signedIn.current?.();
        if (["signed-in", "could-not-follow", "failed", "cancelled"].includes(now.at)) {
          flow.current = undefined;
          return;
        }
      }
    };
    void commands
      .poolSignInStart(pool, profile, provider ?? null)
      .catch(said)
      .then((started) => {
        if (started.status === "error") {
          if (!left) setStep({ at: "not-started", why: started.error });
          return;
        }
        // A strip closed while it started: the sign-in it started is ended at once.
        if (left) {
          void commands.poolSignInCancel(started.data).catch(() => undefined);
          return;
        }
        flow.current = started.data;
        return follow(started.data);
      });
    return () => {
      left = true;
      if (flow.current !== undefined) {
        void commands.poolSignInCancel(flow.current).catch(() => undefined);
        flow.current = undefined;
      }
    };
  }, [pool, profile, provider, attempt]);

  const paste = (event: FormEvent) => {
    event.preventDefault();
    if (flow.current === undefined || code.trim() === "") return;
    setPasteRefused(undefined);
    void commands
      .poolSignInPaste(flow.current, code)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((passed) => {
        if (passed.status === "error") setPasteRefused(passed.error);
        else setCode("");
      });
  };

  const tryAgain = () => {
    setStep({ at: "starting" });
    setPage(undefined);
    setCode("");
    setPasteRefused(undefined);
    setAttempt((n) => n + 1);
  };

  return (
    <SignInStrip
      pool={pool}
      profile={profile}
      harness={harness}
      provider={provider}
      step={step}
      code={code}
      setCode={setCode}
      pasteRefused={pasteRefused}
      onPaste={paste}
      page={page}
      onTryAgain={tryAgain}
      onEnd={onEnd}
      onTerminal={onTerminal}
    />
  );
}

/** What a sign-in strip draws, and what its buttons do. */
export type SignInStripProps = {
  pool: string;
  profile: string;
  harness: string;
  /** The plan it signs in to, for a harness that serves several. */
  provider?: string;
  step: StripStep;
  /** What the code field holds. */
  code: string;
  setCode: (code: string) => void;
  /** Why the core would not pass the code on, where it would not. */
  pasteRefused: string | undefined;
  onPaste: (event: FormEvent) => void;
  /** The page the login opened, to open again. */
  page: string | undefined;
  onTryAgain: () => void;
  onEnd: (signedIn: boolean) => void;
  onTerminal?: () => void;
};

/** **The strip at one step**, as {@link PoolSignIn} drives it: its head, its body, its foot. */
export function SignInStrip(props: SignInStripProps) {
  const { pool, profile, harness, provider, step, onEnd, onTerminal } = props;
  // opencode's Copilot plan signs in by a device code (#4045).
  const device = provider === "github-copilot";
  const now = stepSaid(step, device);
  const over = ["signed-in", "could-not-follow", "failed", "cancelled", "not-started"].includes(
    step.at,
  );
  const who = `${profile} (${harness})`;
  // The keyboard was in the code field, which goes once the code is sent: it goes to the strip,
  // never to the page's start.
  const strip = useRef<HTMLElement>(null);
  const asked = step.at === "code" || step.at === "refused";
  useEffect(() => {
    const at = document.activeElement;
    if (!asked && (at === null || at === document.body)) strip.current?.focus();
  }, [asked, step.at]);
  return (
    <Card
      ref={strip}
      tabIndex={-1}
      className="pool-sign-in"
      aria-label={`Signing ${profile} in on ${pool}`}
      role="group"
    >
      <div className="pool-sign-in-head">
        <strong>
          Signing {profile} in on {pool}
        </strong>
        {/* Each step change, said once to a screen reader. */}
        <span className="ui-setting-help" role="status" aria-live="polite">
          {now ?? (step.at === "signed-in" ? `${profile} is signed in on ${pool}` : "")}
        </span>
      </div>
      <Body {...props} who={who} />
      <div className="pool-sign-in-foot">
        {step.at === "signed-in" ? (
          <Button size="sm" onPress={() => onEnd(true)}>
            Close
          </Button>
        ) : over ? (
          <>
            <Button size="sm" variant="primary" onPress={props.onTryAgain}>
              Try again
            </Button>
            <Button size="sm" variant="ghost" onPress={() => onEnd(false)}>
              Close
            </Button>
          </>
        ) : (
          <Button size="sm" variant="ghost" onPress={() => onEnd(false)}>
            Cancel
          </Button>
        )}
        {onTerminal !== undefined && step.at !== "signed-in" && (
          <Button
            size="sm"
            variant="link"
            tooltip="Opens a shell tab on the runner with the harness's own sign-in typed in"
            onPress={onTerminal}
          >
            Open in terminal
          </Button>
        )}
      </div>
      <p className="ui-setting-help">
        From a terminal:{" "}
        <code>
          purlis profile sign-in {profile} --pool {pool}
          {provider === undefined ? "" : ` --provider ${provider}`}
        </code>
      </p>
    </Card>
  );
}

/** What went wrong at `step`, in one line, where anything did. */
function troubleAt(
  step: StripStep,
  who: string,
  pool: string,
  pasteRefused?: string,
  terminal = false,
) {
  switch (step.at) {
    case "code":
      return pasteRefused;
    case "refused":
      return pasteRefused ?? `The sign-in refused that code: ${step.said}`;
    case "could-not-follow":
      return terminal
        ? "purlis could not follow this sign-in, so it ended it. Sign in with Open in terminal, where the harness asks you itself. It said:"
        : "purlis could not follow this sign-in, so it ended it. Open a project to sign in from a terminal there, where the harness asks you itself. It said:";
    case "failed":
      return `${who} did not sign in on ${pool}: ${step.why}`;
    case "not-started":
      return step.why;
    default:
      return undefined;
  }
}

/** The strip's body at `step`: what went wrong first, in one line, then the step's own. */
function Body(props: SignInStripProps & { who: string }) {
  const trouble = troubleAt(
    props.step,
    props.who,
    props.pool,
    props.pasteRefused,
    props.onTerminal !== undefined,
  );
  return (
    <>
      {trouble !== undefined && (
        <p className="trouble said-in-full" role="alert">
          {trouble}
        </p>
      )}
      <StepBody {...props} />
    </>
  );
}

function StepBody({
  step,
  pool,
  who,
  code,
  setCode,
  onPaste,
  page,
  provider,
  onTerminal,
}: SignInStripProps & { who: string }) {
  const again = page !== undefined && (
    <Button size="sm" variant="ghost" onPress={() => void openUrl(page).catch(() => undefined)}>
      Open the page again
    </Button>
  );
  switch (step.at) {
    case "starting":
    case "open":
      return (
        <p className="pending" aria-busy="true">
          {provider === "github-copilot"
            ? `Starting the sign-in on ${pool}'s runner…`
            : `Starting the sign-in on ${pool}'s runner, then opening its page in your browser…`}
        </p>
      );
    case "device":
      return (
        <DeviceCode
          url={step.url}
          code={step.code}
          who={who}
          pool={pool}
          terminal={onTerminal !== undefined}
        />
      );
    case "code":
    case "refused":
      return (
        <>
          <p>
            Sign in on the page that opened in your browser. It shows a code at the end: paste it
            here.
          </p>
          <form className="pool-sign-in-paste" onSubmit={onPaste}>
            <TextField
              label="Paste code"
              value={code}
              onChange={setCode}
              autoFocus
              autoComplete="off"
              spellCheck={false}
            />
            <Button type="submit" variant="primary" isDisabled={code.trim() === ""}>
              Sign in
            </Button>
          </form>
          <p className="ui-setting-help">
            The code signs in {who} on {pool}&apos;s runner only. Nothing of this Mac&apos;s sign-in
            goes there.
          </p>
          {again}
        </>
      );
    case "checking":
      return (
        <p className="pending" aria-busy="true">
          Asking {pool}&apos;s runner whether {who} is signed in…
        </p>
      );
    case "signed-in":
      return (
        <p>
          <Badge variant="ok">Signed in</Badge> {who} is signed in on {pool}.
        </p>
      );
    case "could-not-follow":
      return <pre className="pool-sign-in-said">{step.said}</pre>;
    case "cancelled":
      return <p className="ui-setting-help">The sign-in was cancelled; nothing changed there.</p>;
    default:
      return null;
  }
}

/**
 * **A device code, then its page**, the window's device-code pattern (as `ForgeDeviceSignIn`
 * shows it): the code first, large, with Copy code, and the page opened only when the person asks.
 */
function DeviceCode({
  url,
  code,
  who,
  pool,
  terminal,
}: {
  url: string;
  code: string;
  who: string;
  pool: string;
  /** Whether Open in terminal is there, for GitHub Enterprise. */
  terminal: boolean;
}) {
  const [copied, setCopied] = useState<boolean>();
  const copy = () => {
    const put = navigator.clipboard?.writeText(code);
    if (!put) {
      setCopied(false);
      return;
    }
    void put.then(
      () => setCopied(true),
      () => setCopied(false),
    );
  };
  return (
    <>
      <p>
        Enter this code on GitHub&apos;s page. GitHub should ask only to read your profile, for
        opencode&apos;s Copilot sign-in. If it names another app, or asks for your repositories or
        organisations, cancel there and here.
      </p>
      <Card as="p" variant="ask" className="forge-device-code">
        <strong>
          <code aria-label="The code to enter on the page">{code}</code>
        </strong>
      </Card>
      <SettingActions>
        <Button variant="primary" onPress={() => void openUrl(url).catch(() => undefined)}>
          Open page
        </Button>
        <Button onPress={copy}>Copy code</Button>
      </SettingActions>
      {copied !== undefined && (
        <p className="ui-setting-help" role="status">
          {copied ? "The code is on the clipboard." : "purlis could not copy the code."}
        </p>
      )}
      <p className="ui-setting-help">
        Or go to <code>{url}</code> on any device. This signs in {who} on {pool}&apos;s runner only,
        on GitHub.com{terminal ? ": for GitHub Enterprise, use Open in terminal." : "."}
      </p>
      <p className="pending" aria-busy="true">
        Waiting for you to approve it there…
      </p>
    </>
  );
}
