// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, LoaderCircle } from "lucide-react";
import {
  commands,
  type IdentitiesShown,
  type IdentityFinished,
  type IdentityRemoved,
  type IdentityStand,
  type PlaneId,
} from "./bindings";
import { Notice } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { Fact, Facts } from "./ui/card/Facts";
import { Checkbox } from "./ui/choice/Checkbox";
import { AlertDialog } from "./ui/dialog/Dialog";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { TextField } from "./ui/field/Field";
import { Select } from "./ui/select/Select";

/**
 * **The agent identity flow** (#4485, rulings on #4464): one component, used by the Agent identity
 * tab and by Pool setup's step (#4490, Q40), as the accepted prototype draws it. One step, one main
 * action, what the button does said before it is pressed.
 *
 * **Nothing secret reaches this page.** It names an owner, an App's public client id and personas.
 * GitHub's address, its code and the App's key stay in the app's process.
 *
 * It resumes from what is found: an App whose key is kept and whose lines Finish hasn't written is
 * in `shown.pending`, so closing the tab or the app after GitHub made the App loses nothing. While
 * GitHub's page is open the try lives in the app: a new one stops it (`Tries`).
 *
 * **Finish is the approval** (Q11): it sends the lines this page shows, and writes only those.
 */

/** How long it rests between asking GitHub whether the App is installed yet. */
const RECHECK_MS = 4000;

/** What stands for the client id before there is an App. */
const NO_APP_YET = "<client id>";

/** What a command answered, or a rejection holding the sentence it refused with. */
export async function answer<T>(
  asked: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
): Promise<T> {
  const said = await asked;
  if (said.status === "error") throw new Error(said.error);
  return said.data;
}

/** The sentence a rejection holds. */
export function whyOf(why: unknown): string {
  return why instanceof Error ? why.message : String(why);
}

export const Out = () => <ExternalLink aria-hidden size={14} />;

/** A page of GitHub's, opened in the person's browser, or `onFail` told it didn't open. */
export function openPage(page: string, onFail: (why: string) => void) {
  void openUrl(page).catch(() => onFail("The browser didn't open. Try again."));
}

/** The heading of a screen. It takes the focus when the person moved to it, not as the tab opens. */
function Head({ title, lead, take }: { title: string; lead?: ReactNode; take: boolean }) {
  const ref = useRef<HTMLHeadingElement>(null);
  const taking = useRef(take);
  useEffect(() => {
    if (taking.current) ref.current?.focus();
  }, []);
  return (
    <header className="agent-identity-head">
      <h2 ref={ref} tabIndex={-1} className="agent-identity-title">
        {title}
      </h2>
      {lead && <p className="agent-identity-lead">{lead}</p>}
    </header>
  );
}

function Action({
  does,
  main,
  onMain,
  busy,
  why,
  icon,
  children,
}: {
  does?: string;
  main?: string;
  onMain?: () => void;
  busy?: boolean;
  /** Why the main button can't be pressed yet. */
  why?: string;
  icon?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <section className="agent-identity-action" aria-label="What to do next">
      {does && <p className="agent-identity-does">{does}</p>}
      <ButtonRow size="lg">
        {main && (
          <Button
            variant="primary"
            onPress={() => {
              if (!busy) onMain?.();
            }}
            isDisabled={why !== undefined}
            tooltip={why}
            aria-disabled={busy || undefined}
            aria-busy={busy}
            icon={icon}
          >
            {main}
          </Button>
        )}
        {children}
      </ButtonRow>
    </section>
  );
}

function Trouble({
  cause,
  title,
  children,
  onDismiss,
}: {
  cause: string;
  title: string;
  children: ReactNode;
  onDismiss: () => void;
}) {
  return (
    <Notice cause={cause} tone="trouble" title={title} onDismiss={onDismiss}>
      {children}
    </Notice>
  );
}

function Waiting({ children }: { children: ReactNode }) {
  return (
    <p className="agent-identity-waiting" role="status">
      <LoaderCircle aria-hidden size={14} className="spinning" /> {children}
    </p>
  );
}

function Lines({ text, label }: { text: string; label: string }) {
  const id = useId();
  return (
    <figure className="agent-identity-code">
      <figcaption id={id}>{label}</figcaption>
      <pre tabIndex={0} role="group" aria-labelledby={id}>
        <code>{text}</code>
      </pre>
    </figure>
  );
}

/** Who uses it (Q4): the project's personas, each one tick. One in `fixed` stays ticked. */
export function WhoUses({
  personas,
  ticked,
  onChange,
  off,
  fixed = [],
}: {
  personas: readonly string[];
  ticked: readonly string[];
  onChange: (to: string[]) => void;
  off?: boolean;
  fixed?: readonly string[];
}) {
  const id = useId();
  return (
    <>
      <h3 className="agent-identity-h3" id={id}>
        Who uses it
      </h3>
      <div className="agent-identity-who" role="group" aria-labelledby={id}>
        {personas.map((name) => (
          <Checkbox
            key={name}
            isSelected={ticked.includes(name)}
            isDisabled={off === true || fixed.includes(name)}
            onChange={(on) =>
              onChange(on ? [...ticked, name] : ticked.filter((one) => one !== name))
            }
          >
            {name}
          </Checkbox>
        ))}
      </div>
    </>
  );
}

const OTHER_ORG = "other organisation";
const MY_ACCOUNT = "my personal account";

/** Who the App is for, and what it may do: kept across a try that didn't finish. */
type Choice = { chosen: string; typed: string; workflows: boolean };

function firstChoice(shown: IdentitiesShown): Choice {
  const first = shown.owners.find((one) => !one.personal) ?? shown.owners[0];
  return { chosen: first ? first.name : OTHER_ORG, typed: "", workflows: false };
}

/** The first screen: why, who holds the App, and who uses it. */
function Start({
  shown,
  ticked,
  onTick,
  choice,
  onChoice,
  onCreate,
  trouble,
  onDismiss,
  take,
  extra,
}: {
  shown: IdentitiesShown;
  ticked: readonly string[];
  onTick: (to: string[]) => void;
  choice: Choice;
  onChoice: (to: Choice) => void;
  onCreate: (owner: string, personal: boolean) => void;
  trouble: string | null;
  onDismiss: () => void;
  take: boolean;
  extra?: ReactNode;
}) {
  const { chosen, typed, workflows } = choice;
  const [preview, setPreview] = useState("");
  useEffect(() => {
    if (ticked.length === 0) return;
    let current = true;
    void answer(commands.agentIdentityLines(NO_APP_YET, [...ticked])).then(
      (text) => {
        if (current) setPreview(text);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [ticked]);
  const owner = shown.owners.find((one) => one.name === chosen);
  const asks = chosen === OTHER_ORG || chosen === MY_ACCOUNT;
  const personal = chosen === MY_ACCOUNT || owner?.personal === true;
  const name = asks ? typed.trim() : chosen;
  const organisations = shown.owners.filter((one) => !one.personal).map((one) => one.name);
  const why =
    shown.personas.length === 0
      ? "This project has no personas yet."
      : ticked.length === 0
        ? "Tick who uses it first."
        : name.length === 0
          ? "Name who owns the App, under Options."
          : undefined;
  const where = personal ? "your own account" : name || "the organisation";
  const recommended = shown.owners.find((one) => !one.personal) ?? shown.owners[0];
  return (
    <>
      <Head
        take={take}
        title="Give the agent its own GitHub identity"
        lead="So that gh and git push work in a chat. You do this once, then it stays out of your way."
      />
      <p className="agent-identity-term">
        <strong>An agent identity</strong> is what a chat acts as on GitHub. It's a separate
        identity for the agent, with its own limits. Your own sign-in never reaches a chat, so the
        agent can never act as you.
      </p>
      {trouble && (
        <Trouble cause="agent-identity-not-made" title="The App wasn't made" onDismiss={onDismiss}>
          {trouble}
        </Trouble>
      )}
      <Facts aria-label="What purlis found">
        {shown.owners.length > 0 && (
          <Fact label="Your repos are on">
            github.com, in {organisations.join(", ") || "your own account"}
          </Fact>
        )}
        <Fact label="It may">
          Read and write code, pull requests and issues
          {workflows ? ", and change CI workflows" : ""}
        </Fact>
      </Facts>
      {shown.personas.length === 0 ? (
        <p className="agent-identity-term">
          This project has no personas yet. Add one from the palette, then come back here.
        </p>
      ) : (
        <WhoUses personas={shown.personas} ticked={ticked} onChange={onTick} />
      )}
      <Disclosure
        label="Options"
        level={3}
        className="agent-identity-fold"
        defaultExpanded={shown.owners.length === 0}
      >
        <div className="agent-identity-fold-body">
          <Checkbox
            isSelected={workflows}
            onChange={(on) => onChoice({ ...choice, workflows: on })}
            description="Off by default. Most agents don't need it. Tokens a cluster makes never change workflow files, whatever this says."
          >
            Also let it change CI workflows
          </Checkbox>
          <Select
            aria-label="Who owns the App"
            value={chosen}
            onValueChange={(to) => onChoice({ ...choice, chosen: to })}
            options={[
              ...shown.owners.map((one) => ({
                value: one.name,
                label: `Owned by: ${one.name}${one === recommended ? ". Recommended" : ""}`,
              })),
              { value: OTHER_ORG, label: "Owned by: another organisation" },
              ...(shown.owners.some((one) => one.personal)
                ? []
                : [{ value: MY_ACCOUNT, label: "Owned by: my personal account" }]),
            ]}
          />
          {asks && (
            <TextField
              label={chosen === OTHER_ORG ? "Organisation" : "Your GitHub account"}
              value={typed}
              onChange={(to) => onChoice({ ...choice, typed: to })}
            />
          )}
        </div>
      </Disclosure>
      {preview && (
        <Disclosure label="See what this does" level={3} className="agent-identity-fold">
          <div className="agent-identity-fold-body">
            <p className="agent-identity-term">
              GitHub's page does the rest. If you'd rather make the App by hand, add a line like
              this and purlis shows it here.
            </p>
            <Lines text={preview} label="The lines this ends up adding" />
          </div>
        </Disclosure>
      )}
      <Action
        does={`Opens GitHub in your browser to create an App for ${where}. You confirm it there and choose its repos.`}
        main="Create the GitHub App"
        icon={<Out />}
        why={why}
        onMain={() => onCreate(name, personal)}
      >
        {extra}
      </Action>
    </>
  );
}

/** Waits on GitHub's page, where the App is made. */
function OnGitHub({
  onAgain,
  onStop,
  trouble,
  onDismiss,
  take,
}: {
  onAgain: () => void;
  onStop: () => void;
  trouble: string | null;
  onDismiss: () => void;
  take: boolean;
}) {
  return (
    <>
      <Head
        take={take}
        title="Finish on GitHub"
        lead="GitHub's page is open in your browser. You're making a GitHub App, then choosing its repos."
      />
      {trouble && (
        <Trouble
          cause="agent-identity-page"
          title="GitHub's page didn't open"
          onDismiss={onDismiss}
        >
          {trouble}
        </Trouble>
      )}
      <ol className="agent-identity-steps" aria-label="On GitHub's page">
        <li>Press Create GitHub App on the page.</li>
        <li>Install it on your repos. Choose all repos, or only some.</li>
      </ol>
      <Waiting>Waiting for GitHub. This moves on by itself.</Waiting>
      <Action>
        <Button icon={<Out />} onPress={onAgain}>
          Open the page again
        </Button>
        <Button variant="ghost" onPress={onStop}>
          Start over
        </Button>
      </Action>
    </>
  );
}

/** The App is made and has no repos yet. */
function ChooseRepos({
  stand,
  trouble,
  onDismiss,
  onOpenFailed,
  onStartOver,
  take,
}: {
  stand: IdentityStand;
  trouble: string | null;
  onDismiss: () => void;
  onOpenFailed: (why: string) => void;
  onStartOver: () => void;
  take: boolean;
}) {
  return (
    <>
      <Head
        take={take}
        title="Choose its repos"
        lead={`GitHub made ${stand.acts_as}. Its key went straight into your vault, and nothing shows it. Now install it on your repos.`}
      />
      {trouble && (
        <Trouble cause="agent-identity-stand" title="GitHub didn't answer" onDismiss={onDismiss}>
          {trouble}
        </Trouble>
      )}
      <Waiting>Waiting for GitHub. This moves on by itself.</Waiting>
      <Action>
        <Button icon={<Out />} onPress={() => openPage(stand.install_page, onOpenFailed)}>
          Open the page again
        </Button>
        <Button variant="ghost" onPress={onStartOver}>
          Start over
        </Button>
      </Action>
    </>
  );
}

/** One last look (Q11): Finish is the approval, and these are its exact lines. */
function LastLook({
  stand,
  personas,
  ticked,
  onTick,
  lines,
  busy,
  trouble,
  onDismiss,
  onFinish,
  onStartOver,
  take,
}: {
  stand: IdentityStand;
  personas: readonly string[];
  ticked: readonly string[];
  onTick: (to: string[]) => void;
  /** The exact lines for `ticked`, or null until they're here. */
  lines: string | null;
  busy: boolean;
  trouble: string | null;
  onDismiss: () => void;
  onFinish: () => void;
  onStartOver: () => void;
  take: boolean;
}) {
  return (
    <>
      <Head
        take={take}
        title="One last look"
        lead="GitHub made the App. Its key went straight into your vault, and nothing shows it."
      />
      {trouble && (
        <Trouble cause="agent-identity-finish" title="That didn't finish" onDismiss={onDismiss}>
          {trouble}
        </Trouble>
      )}
      <Facts aria-label="What was made">
        <Fact label="Acts as">{stand.acts_as}</Fact>
        <Fact label="Owned by">{stand.owner}</Fact>
        <Fact label="May change">
          Code, pull requests and issues{stand.workflows ? ", and CI workflows" : ""}
        </Fact>
      </Facts>
      <WhoUses personas={personas} ticked={ticked} onChange={onTick} off={busy} />
      {lines !== null && <Lines text={lines} label="The lines Finish adds to your project" />}
      <Action
        does={`Adds these lines for ${ticked.join(", ") || "nobody yet"}, gets a real short-lived token and runs read-only checks. A persona with more on its list keeps its own Allow.`}
        main="Finish"
        busy={busy}
        why={
          ticked.length === 0
            ? "Tick who uses it first."
            : lines === null
              ? "The lines are still being read."
              : undefined
        }
        onMain={onFinish}
      >
        <Button variant="ghost" onPress={onStartOver}>
          Start over
        </Button>
      </Action>
    </>
  );
}

/** Asks before an App the person waited on is given up: its key goes. */
export function StartOverAsk({
  onClose,
  onStartOver,
}: {
  onClose: () => void;
  onStartOver: () => void;
}) {
  return (
    <AlertDialog
      onClose={onClose}
      title="Start over?"
      description={
        <p className="came-back">
          This App's key goes from your vault, and nothing used it yet. The App itself stays on
          GitHub until you delete it there.
        </p>
      }
      main={{ children: "Start over", variant: "danger", onPress: onStartOver }}
    />
  );
}

type Screen =
  | { at: "start" }
  | { at: "browser"; attempt: string }
  | { at: "made"; app: string; stand: IdentityStand | null; busy: boolean };

/**
 * **The flow itself.** `asking` is the persona ticked to begin with (Q4). `onDone` hears Finish, and
 * `onAbandoned` an App the person gave up on, with what remains on GitHub. `extra` is a further
 * button beside the main one, as Pool setup's Skip for now.
 */
export function AgentIdentityFlow({
  plane,
  shown,
  asking,
  onDone,
  onAbandoned,
  extra,
}: {
  plane: PlaneId;
  shown: IdentitiesShown;
  asking?: string;
  onDone: (done: IdentityFinished) => void;
  onAbandoned: (left: IdentityRemoved) => void;
  extra?: ReactNode;
}) {
  const waiting = shown.pending[0];
  const [opened] = useState<Screen>(() =>
    waiting ? { at: "made", app: waiting.app, stand: waiting.stand, busy: false } : { at: "start" },
  );
  const [screen, setScreen] = useState<Screen>(opened);
  // The first screen drawn leaves the focus where it is: every later one takes it.
  const take = screen !== opened;
  const [trouble, setTrouble] = useState<string | null>(waiting?.trouble ?? null);
  const [ticked, setTicked] = useState<string[]>(() =>
    asking && shown.personas.includes(asking) ? [asking] : shown.personas.slice(0, 1),
  );
  const [choice, setChoice] = useState<Choice>(() => firstChoice(shown));
  const [lines, setLines] = useState<{ for: string[]; text: string } | null>(null);
  const [confirming, setConfirming] = useState(false);
  const attempt = useRef<string | null>(null);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  const app = screen.at === "made" ? screen.app : null;
  const installed = screen.at === "made" && screen.stand?.installed === true;

  // Moves on by itself as the App is installed, one ask at a time.
  useEffect(() => {
    if (app === null || installed) return;
    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ask = () => {
      void answer(commands.agentIdentityStand(plane, app))
        .then(
          (stand) => {
            if (!current) return;
            setTrouble(null);
            setScreen((now) => (now.at === "made" ? { ...now, stand } : now));
          },
          (why: unknown) => {
            if (current) setTrouble(whyOf(why));
          },
        )
        .finally(() => {
          if (current) timer = setTimeout(ask, RECHECK_MS);
        });
    };
    ask();
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [plane, app, installed]);

  // The lines are fetched for one list of ticks, and count only for that list.
  useEffect(() => {
    if (app === null || ticked.length === 0) return;
    let current = true;
    void answer(commands.agentIdentityLines(app, ticked)).then(
      (text) => {
        if (current) setLines({ for: ticked, text });
      },
      (why: unknown) => {
        if (current) setTrouble(whyOf(why));
      },
    );
    return () => {
      current = false;
    };
  }, [app, ticked]);

  const shownLines = lines?.for === ticked ? lines.text : null;

  const create = (owner: string, personal: boolean) => {
    const mine = crypto.randomUUID();
    attempt.current = mine;
    setTrouble(null);
    setScreen({ at: "browser", attempt: mine });
    void answer(commands.agentIdentityCreate(plane, mine, owner, personal, choice.workflows)).then(
      (made) => {
        if (!live.current || attempt.current !== mine) return;
        setScreen({ at: "made", app: made, stand: null, busy: false });
      },
      (why: unknown) => {
        if (!live.current || attempt.current !== mine) return;
        setTrouble(whyOf(why));
        setScreen({ at: "start" });
      },
    );
  };

  const stop = (running: string) => {
    attempt.current = null;
    setTrouble(null);
    setScreen({ at: "start" });
    void commands.agentIdentityCreateCancel(running);
  };

  const finish = (made: string, shownNow: string) => {
    setTrouble(null);
    setScreen((now) => (now.at === "made" ? { ...now, busy: true } : now));
    void answer(commands.agentIdentityFinish(plane, made, ticked, shownNow)).then(
      onDone,
      (why: unknown) => {
        if (!live.current) return;
        setTrouble(whyOf(why));
        setScreen((now) => (now.at === "made" ? { ...now, busy: false } : now));
      },
    );
  };

  const abandon = (made: string) => {
    setConfirming(false);
    setTrouble(null);
    void answer(commands.agentIdentityRemove(plane, made)).then(onAbandoned, (why: unknown) => {
      if (live.current) setTrouble(whyOf(why));
    });
  };

  const dismiss = () => setTrouble(null);

  if (screen.at === "start")
    return (
      <Start
        shown={shown}
        ticked={ticked}
        onTick={setTicked}
        choice={choice}
        onChoice={setChoice}
        onCreate={create}
        trouble={trouble}
        onDismiss={dismiss}
        take={take}
        extra={extra}
      />
    );
  if (screen.at === "browser")
    return (
      <OnGitHub
        onAgain={() =>
          void answer(commands.agentIdentityReopen()).catch((why: unknown) =>
            setTrouble(whyOf(why)),
          )
        }
        onStop={() => stop(screen.attempt)}
        trouble={trouble}
        onDismiss={dismiss}
        take={take}
      />
    );
  const startOver = () => setConfirming(true);
  const asked = confirming && (
    <StartOverAsk onClose={() => setConfirming(false)} onStartOver={() => abandon(screen.app)} />
  );
  if (screen.stand === null)
    return (
      <>
        <Head take={take} title="Reading the App from GitHub…" />
        {trouble && (
          <Trouble cause="agent-identity-stand" title="GitHub didn't answer" onDismiss={dismiss}>
            {trouble}
          </Trouble>
        )}
        <Waiting>Asking again by itself.</Waiting>
        <Action>
          <Button variant="ghost" onPress={startOver}>
            Start over
          </Button>
        </Action>
        {asked}
      </>
    );
  if (!screen.stand.installed)
    return (
      <>
        <ChooseRepos
          stand={screen.stand}
          trouble={trouble}
          onDismiss={dismiss}
          onOpenFailed={setTrouble}
          onStartOver={startOver}
          take={take}
        />
        {asked}
      </>
    );
  return (
    <>
      <LastLook
        stand={screen.stand}
        personas={shown.personas}
        ticked={ticked}
        onTick={setTicked}
        lines={shownLines}
        busy={screen.busy}
        trouble={trouble}
        onDismiss={dismiss}
        onFinish={() => {
          if (shownLines !== null) finish(screen.app, shownLines);
        }}
        onStartOver={startOver}
        take={take}
      />
      {asked}
    </>
  );
}
