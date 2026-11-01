// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  commands,
  type PoolLimitOffer,
  type PoolSignInStep,
  type PoolSignInTerminal,
} from "../bindings";
import { RaiseLimit } from "../RaiseLimit";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { TextField } from "../ui/field/Field";
import { SettingActions } from "./components";
import { SignInTerminal, type Printer } from "./SignInTerminal";

/**
 * Where a sign-in is, as the strip draws it: starting, one of the core's steps, the device code
 * the person asked for running in its terminal, or refused.
 */
export type StripStep =
  | { at: "starting" }
  | Exclude<PoolSignInStep, { at: "printed" }>
  | { at: "device-in-terminal" }
  | { at: "not-started"; why: string };

/** A profile's standing on its line while its strip is open (spec #4023 story 29). */
export type SignInStanding = "signing-in" | "in-the-terminal" | "failed" | "signed-in";

/**
 * Whether the login of harness `kind` to `plan` is approved in the browser, with nothing to paste
 * (#4044): Codex's, and opencode's ChatGPT one.
 */
export const approvesIn = (kind: string, plan?: string) =>
  kind === "codex" || (kind === "opencode" && plan === "openai");

/**
 * Whether the sign-in of harness `kind` to the plan `plan` is driven in place: opencode's Copilot
 * (#4045) and its ChatGPT (#4044).
 */
export const drivenPlan = (kind: string, plan: string) =>
  kind === "opencode" && (plan === "github-copilot" || plan === "openai");

/** The size of the strip's terminal: as wide as a login expects, and a few lines shorter. */
const TERMINAL: PoolSignInTerminal = { columns: 80, rows: 16 };

/**
 * What a login approved in the browser says beside the runner's terminal: its device flow prints
 * its own page, which purlis reads nothing of, and both of these logins are OpenAI's.
 */
const CHECK_THE_PAGE =
  "purlis does not check what the runner prints. Enter the code only on a page of openai.com; if it names another app, or asks for more than signing in, cancel there and here.";

/**
 * **The step the strip is on, in words** (spec #4023 story 12): "Step 2 of 3: …". A device code's
 * sign-in (`device`) opens no page as it starts. A login approved in the browser has one step
 * fewer, with nothing to paste (`approves`, #4044).
 */
export function stepSaid(step: StripStep, device = false, approves = false): string | null {
  const steps = approves ? 2 : 3;
  switch (step.at) {
    case "starting":
      if (device) return "Step 1 of 3: Starting the sign-in on the runner";
      return `Step 1 of ${steps}: Opening the sign-in page in your browser`;
    case "open":
      return `Step 1 of ${steps}: Opening the sign-in page in your browser`;
    case "approve":
      return `Step 1 of ${steps}: Approve in your browser`;
    case "device":
      return "Step 2 of 3: Enter the code on the page";
    case "code":
    case "refused":
      return `Step 2 of ${steps}: Paste the code the page shows`;
    case "checking":
      return `Step ${steps} of ${steps}: Checking it is signed in on the runner`;
    case "could-not-follow":
      return "purlis could not follow this sign-in: the runner's own terminal is open below";
    case "device-in-terminal":
      return `Step 1 of ${steps}: Enter the device code the terminal below shows`;
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
 * vendor's own https host, opened when the person asks. Codex's, and opencode's ChatGPT one
 * (#4044), have nothing to paste: the person approves in the page, and purlis carries the
 * browser's answer to the runner. A port taken on this Mac is said, with a device code as the way
 * out, run in the strip's own terminal, so it needs no project (#4106). Nothing of this Mac's
 * sign-in goes to the runner (ADR 0095).
 *
 * Each step is said by the core (`pool_sign_in_next`); the strip draws it, says which step it is
 * on, moves the keyboard to the code field when it appears, and says each step change to a
 * screen reader. Cancel, closing the strip, or trying again ends the login on the runner too, as
 * its input closes (#4043).
 *
 * **Where purlis cannot follow the login, the person can** (#4043): the core says so, and the
 * login is started again in a terminal the runner makes, drawn here ({@link SignInTerminal}). The
 * core reads nothing of it, and asks the runner's status check when it ends.
 */
export function PoolSignIn({
  pool,
  profile,
  kind,
  harness,
  provider,
  onEnd,
  onTerminal,
  onSignedIn,
  onStanding,
}: {
  pool: string;
  profile: string;
  /** Its harness, as a profile's kind names it: `claude`. */
  kind: string;
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
  /** Told the standing its line says, as it changes. */
  onStanding?: (standing: SignInStanding) => void;
}) {
  const [step, setStep] = useState<StripStep>({ at: "starting" });
  const [page, setPage] = useState<string>();
  const [code, setCode] = useState("");
  const [pasteRefused, setPasteRefused] = useState<string>();
  const [attempt, setAttempt] = useState(0);
  /** What to raise the pool's limit to, where the last try ran out of it (spec #4275). */
  const [limitOffer, setLimitOffer] = useState<PoolLimitOffer>();
  // Why this try runs in the strip's terminal, where it does: purlis could not follow the one
  // before, or the person asked for a device code (#4106).
  const [terminalFor, setTerminalFor] = useState<"lost" | "device">();
  const inTerminal = terminalFor !== undefined;
  const flow = useRef<number>(undefined);
  // What is typed goes on one call at a time, per sign-in, so it reaches the login in order (#4123).
  const typing = useRef<Promise<void>>(Promise.resolve());
  const signedIn = useRef(onSignedIn);
  useEffect(() => {
    signedIn.current = onSignedIn;
  }, [onSignedIn]);
  // How to print in the terminal once it is drawn, and what was printed before it was (a step
  // the core said while the strip's terminal was not yet drawn).
  const printing = useRef<{ held: string[]; print?: (text: string) => void }>({ held: [] });

  const standing = standingAt(step);
  useEffect(() => onStanding?.(standing), [onStanding, standing]);

  const printer: Printer = useCallback((print) => {
    printing.current.print = print;
    if (print === undefined) return;
    printing.current.held.forEach(print);
    printing.current.held = [];
  }, []);

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
          // Refused only once it is over: nothing it opened is any use now.
          setPage(undefined);
          setStep({ at: "failed", why: next.error, stopped: true });
          return;
        }
        if (next.data === null) continue;
        const now = next.data;
        if (now.at === "printed") {
          const { print, held } = printing.current;
          if (print === undefined) held.push(now.text);
          else print(now.text);
          continue;
        }
        if (now.at === "open" || now.at === "approve") {
          setPage(now.url);
          void openUrl(now.url).catch(() => undefined);
        }
        if (now.at === "failed" && now.stopped) setPage(undefined);
        setStep(now);
        if (now.at === "could-not-follow") {
          flow.current = undefined;
          // The page the login opened is over with it.
          setPage(undefined);
          setTerminalFor("lost");
          return;
        }
        if (now.at === "signed-in") signedIn.current?.();
        if (now.at === "failed") {
          const why = now.why;
          void commands
            .poolLimitOfferForMove(pool, why)
            .catch(() => null)
            .then((offer) => {
              if (!left) setLimitOffer(offer ?? undefined);
            });
        }
        if (["signed-in", "port-taken", "failed", "cancelled"].includes(now.at)) {
          flow.current = undefined;
          return;
        }
      }
    };
    void commands
      .poolSignInStart(pool, profile, provider ?? null, inTerminal ? TERMINAL : null)
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
        typing.current = Promise.resolve();
        return follow(started.data);
      });
    return () => {
      left = true;
      if (flow.current !== undefined) {
        void commands.poolSignInCancel(flow.current).catch(() => undefined);
        flow.current = undefined;
      }
    };
  }, [pool, profile, provider, attempt, inTerminal]);

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
    // A sign-in purlis could not follow starts again in the terminal, never at the first step.
    setStep(
      terminalFor === "device"
        ? { at: "device-in-terminal" }
        : terminalFor === "lost"
          ? { at: "could-not-follow", said: "" }
          : { at: "starting" },
    );
    setPage(undefined);
    setCode("");
    setPasteRefused(undefined);
    printing.current.held = [];
    setLimitOffer(undefined);
    setAttempt((n) => n + 1);
  };

  const raiseAndTryAgain = (values: ReadonlyArray<readonly [string, number]>) => {
    void Promise.all(
      values.map(([key, value]) =>
        commands.poolLimitSet(pool, key, value).catch((err: unknown) => ({
          status: "error" as const,
          error: String(err),
        })),
      ),
    ).then((saved) => {
      const refused = saved.find((one) => one.status === "error");
      if (refused?.status === "error") setPasteRefused(refused.error);
      else tryAgain();
    });
  };

  // The port-taken login is over: the core starts the harness's device flow in a terminal.
  // A device flow that failed, its account may have it off: the browser's way is still there.
  const browserAgain = () => {
    setStep({ at: "starting" });
    setPage(undefined);
    setCode("");
    setPasteRefused(undefined);
    printing.current.held = [];
    setTerminalFor(undefined);
  };

  const useDeviceCode = () => {
    setStep({ at: "device-in-terminal" });
    setPage(undefined);
    setPasteRefused(undefined);
    printing.current.held = [];
    setTerminalFor("device");
  };

  const type = (text: string) => {
    const which = flow.current;
    if (which === undefined) return;
    typing.current = typing.current.then(() =>
      commands
        .poolSignInType(which, text)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((typed) => {
          // Refused once that sign-in is over, it is nothing to say.
          if (flow.current === which)
            setPasteRefused(typed.status === "error" ? typed.error : undefined);
        }),
    );
  };

  return (
    <SignInStrip
      pool={pool}
      profile={profile}
      kind={kind}
      harness={harness}
      provider={provider}
      approves={approvesIn(kind, provider)}
      step={step}
      code={code}
      setCode={setCode}
      pasteRefused={pasteRefused}
      onPaste={paste}
      page={page}
      onTryAgain={tryAgain}
      limitOffer={limitOffer}
      onRaiseLimit={raiseAndTryAgain}
      onEnd={onEnd}
      onTerminal={onTerminal}
      terminal={inTerminal ? { printer, onType: type, attempt } : undefined}
      onBrowserAgain={terminalFor === "device" ? browserAgain : undefined}
      onDeviceCode={useDeviceCode}
    />
  );
}

/** The standing `step` gives its line. */
function standingAt(step: StripStep): SignInStanding {
  switch (step.at) {
    case "could-not-follow":
    case "device-in-terminal":
      return "in-the-terminal";
    // A port taken on this Mac ended the sign-in: the person acts on it (#4044).
    case "port-taken":
    case "failed":
    case "not-started":
      return "failed";
    case "signed-in":
      return "signed-in";
    default:
      return "signing-in";
  }
}

/** What a sign-in strip draws, and what its buttons do. */
export type SignInStripProps = {
  pool: string;
  profile: string;
  /** Its harness, as a profile's kind names it. */
  kind: string;
  harness: string;
  /** The plan it signs in to, for a harness that serves several. */
  provider?: string;
  /** Whether its login is approved in the browser, with nothing to paste. */
  approves?: boolean;
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
  /** What to raise the pool's limit to, where the last try ran out of it (spec #4275). */
  limitOffer?: PoolLimitOffer;
  onRaiseLimit?: (values: ReadonlyArray<readonly [string, number]>) => void;
  onEnd: (signedIn: boolean) => void;
  onTerminal?: () => void;
  /** The strip's terminal, while a login runs in it or just ended there. */
  terminal?: { printer: Printer; onType: (text: string) => void; attempt: number };
  /** "Try the browser", after a device code that did not sign in (#4106). */
  onBrowserAgain?: () => void;
  /** "Use a device code": the harness's device flow, in the strip's terminal (#4106). */
  onDeviceCode: () => void;
};

/** **The strip at one step**, as {@link PoolSignIn} drives it: its head, its body, its foot. */
export function SignInStrip(props: SignInStripProps) {
  const { pool, profile, harness, provider, step, onEnd, onTerminal } = props;
  // opencode's Copilot plan signs in by a device code (#4045).
  const device = provider === "github-copilot";
  const now = stepSaid(step, device, props.approves);
  const over = ["signed-in", "port-taken", "failed", "cancelled", "not-started"].includes(step.at);
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
            {/* The login ended, so the page it opened is over too: this starts another. */}
            <Button size="sm" variant="primary" onPress={props.onTryAgain}>
              {step.at === "failed" && props.page !== undefined
                ? "Open the page again"
                : "Try again"}
            </Button>
            {props.onBrowserAgain !== undefined && (
              <Button size="sm" onPress={props.onBrowserAgain}>
                Try the browser
              </Button>
            )}
            <Button size="sm" variant="ghost" onPress={() => onEnd(false)}>
              Close
            </Button>
          </>
        ) : step.at === "checking" ? null : (
          // While it is checked, the login on the runner has already ended: nothing is left to
          // cancel, and what it signed in stays.
          <Button size="sm" variant="ghost" onPress={() => onEnd(false)}>
            Cancel
          </Button>
        )}
        {onTerminal !== undefined && step.at !== "signed-in" && step.at !== "checking" && (
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
      {/* A login approved in the browser has no way back from a terminal here (#4044). */}
      {!props.approves && (
        <p className="ui-setting-help">
          From a terminal:{" "}
          <code>
            purlis profile sign-in {profile} --pool {pool}
            {provider === undefined ? "" : ` --provider ${provider}`}
          </code>
        </p>
      )}
    </Card>
  );
}

/** What went wrong at `step`, in one line, where anything did. */
function troubleAt(step: StripStep, who: string, pool: string, pasteRefused?: string) {
  switch (step.at) {
    case "code":
    case "could-not-follow":
    case "device-in-terminal":
      return pasteRefused;
    case "refused":
      return pasteRefused ?? `The sign-in refused that code: ${step.said}`;
    case "port-taken":
      return `Port ${step.port} is in use on this Mac, so your browser's answer cannot reach ${pool}'s runner. Close what is using it and try again, or use a device code.`;
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
  const trouble = troubleAt(props.step, props.who, props.pool, props.pasteRefused);
  return (
    <>
      {trouble !== undefined && (
        <p className="trouble said-in-full" role="alert">
          {trouble}
        </p>
      )}
      {props.limitOffer !== undefined && props.onRaiseLimit !== undefined && (
        <RaiseLimit
          offer={props.limitOffer}
          action="Save to the pool and try again"
          refused={props.pasteRefused}
          onApply={props.onRaiseLimit}
        />
      )}
      <StepBody {...props} />
      {props.terminal !== undefined && props.step.at !== "not-started" && (
        <SignInTerminal
          // Each try is a screen of its own: nothing of the login before it stays to be copied.
          key={props.terminal.attempt}
          label={`The runner's terminal on ${props.pool}`}
          size={TERMINAL}
          printer={props.terminal.printer}
          onType={props.terminal.onType}
          live={props.step.at === "could-not-follow" || props.step.at === "device-in-terminal"}
        />
      )}
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
  kind,
  provider,
  approves,
  onTerminal,
  onDeviceCode,
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
    case "approve":
      return (
        <>
          <p>
            Approve the sign-in on the page that opened in your browser. There is nothing to paste:
            purlis carries your browser&apos;s answer to {pool}&apos;s runner.
          </p>
          <p className="ui-setting-help">
            This signs in {who} on {pool}&apos;s runner only. Nothing of this Mac&apos;s sign-in
            goes there.
          </p>
          {again}
        </>
      );
    case "port-taken":
      return (
        <>
          <Button size="sm" onPress={onDeviceCode}>
            Use a device code
          </Button>
          <p className="ui-setting-help">
            {kind === "codex" ? (
              <>
                Runs <code>codex login --device-auth</code> on the runner, here in this strip.
              </>
            ) : (
              "Runs its device-code sign-in on the runner, here in this strip."
            )}
          </p>
        </>
      );
    case "device-in-terminal":
      return (
        <>
          <p>
            The runner&apos;s terminal below runs the device-code sign-in. Copy the page it prints
            to your browser and enter its code there; purlis checks when it ends.
          </p>
          <p className="ui-setting-help">{CHECK_THE_PAGE}</p>
        </>
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
      return (
        <>
          <p>
            purlis could not follow this sign-in, so it started it again in the runner&apos;s own
            terminal below, and a page it opened before is over. Finish it there; purlis checks when
            it ends.
          </p>
          {approves && <p className="ui-setting-help">{CHECK_THE_PAGE}</p>}
        </>
      );
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
