// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import {
  commands,
  type Allowed,
  type BlockReport,
  type GrantLevel,
  type GrantWhat,
  type KubeContext,
  type Offered,
  type OpenChat,
  type Shown,
} from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { useReadOnlySaid } from "./ReadOnlyLine";
import { sandboxCommandReturned } from "./sandboxAsked";
import { asksMoved } from "./asks";
import { inTheWindow } from "./personaHostsAllow";
import { eachHost, hostsOf, inertly, isAnAsk, matched, type HeldBlock } from "./sandboxBlocks";
import { SandboxChoose, chooseSaid, mayChoose } from "./SandboxChoose";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { moreRefusedSaid, useRefusedMore } from "./refusedMore";
import { RestartEndsLine } from "./RestartEnds";
import { Field, SettingRow } from "./settings/components";
import { Button, ButtonRow } from "./ui/button/Button";
import { Card } from "./ui/card/Card";

/**
 * **What a chat's sandbox blocked, on its tab** (#1338): the operation and the kind of path or
 * host, in the core's words, and nothing of what the chat ran.
 *
 * A block of **purlis's own** operation is a purlis bug, and says so. It offers **Report…**,
 * which shows the draft the core makes from the block alone — the operation, the kind and the
 * versions — and files it only on **File report**: by the app, as the person, with purlis's own
 * sign-in to GitHub (#806), never from inside the chat. Nothing is sent before that press.
 *
 * A block of the chat's own work is **never a dead end** (#1342): a host or a folder to write
 * offers Allow for this chat, **Always allow…** (every chat of this project on this machine,
 * or for a host everyone in the project) and Keep blocked, with what it would allow shown
 * whole first. The core judges it again and audits it; the window then restarts the chat on its
 * conversation once its turn has ended (`onAllowed`, driven by `PlaneView`), and it is told what
 * was allowed. What is never granted says the way that works; what purlis grants nothing for
 * offers **Start without the sandbox for this chat**, the person's own choice.
 *
 * **Several hosts refused at once are one Notice** (#1637): each host is listed whole, and one
 * Allow allows each of them, every one judged by the core on its own as a single host is, with
 * one restart to take them all. A host that joins the Notice while an answer is on its way is
 * not in that answer: it stays up, asking.
 *
 * **Policy has the last word** (#1343): Allow is offered only at the levels an administrator's
 * policy leaves open (`block.levels`), and where policy forbids every Allow and starting the
 * chat without the sandbox too, nothing is offered — the Notice still says what was blocked,
 * what policy forbids, and who set it, so the person knows whom to ask. Where it removed one
 * choice, or turned asking off, the Notice says so (`block.ruled`, #1666).
 *
 * **A host is asked live** (#1666): where the chat's proxy holds the connection while the
 * person answers (`block.held`), the Notice says it waits, and an Allow lets the same command
 * carry on with nothing restarting (`Allowed.live`). For a host the main button allows it for
 * this project on this machine; **Other scopes…** offers only this chat, or everyone in the
 * project. **Keep blocked** refuses what the proxy holds and tells the chat
 * (`keep_sandbox_block`).
 *
 * **A local socket or a system service offers two answers side by side** (the operator's ruling
 * R2 of 2026-10-13): the narrowest, Allow once, and **Allow on this machine, for every project**,
 * so one press is the last for something every project of the person's uses. Only this chat, and
 * this project alone, stay under **Other scopes…**.
 *
 * **A host allowed already offers no Allow** (#1666's fold-in): it says so, and that this chat
 * takes it once it restarts, with **Restart this chat**.
 *
 * **Hosts past the eight the proxy names are counted** (#1353, `useRefusedMore`): the Notice
 * says how many more were refused, and that no Allow on it reaches them, since it names none.
 *
 * **A host refused while a brokered command ran says the vault it ran with** (#1353,
 * `block.vault`): the person judges whether that vault belongs at that host before allowing it.
 * Its name only, as the core drew it (inertly); never a value, and never a host that carries one.
 *
 * **Drawn from the ask the registry lists for it** (#1695, `asks`): a block a grant can name is
 * a sandbox ask, and its Notice asks only while the registry lists it, so an answer anywhere
 * (here, in the Inbox) clears both. Its answers are the ask's own, in the ask's order and words
 * (#1700: the labels are written once, in the registry), at the levels both the ask and the
 * block leave open. **A hold that ran out is said plainly** (#1709): a connection the proxy held
 * when the block arrived, and holds no more, says that nobody answered in time, and that an
 * Allow now tells the chat to run it again. Once allowed, it offers **Tell the chat to retry**
 * (D-1709-7): a chat gone idle hears that only on its next turn, and purlis never types into a
 * chat by itself, so the press puts a fixed line in its prompt for the person to send.
 *
 * **A socket or a service the block did not name is the person's to choose** (#1884,
 * `block.choose`): **Choose the socket…** stands beside Start without the sandbox and opens the
 * app's own list and a box for a path (`SandboxChoose`). What they choose is judged as a named
 * one is, and the Notice that names it then offers Allow once, for this chat and always.
 *
 * **A block the chat only read back is a quiet line** (#1884, `block.quiet`): found in the
 * output of a command that ran earlier, by a tool that ran nothing. It says so, asks nothing,
 * and is put away with Dismiss; **Show what you can do** takes it up, and it is then the
 * block's own Notice, with every way out it has.
 */
export function SandboxBlockNotice({
  block,
  asks = [],
  more,
  onDismiss,
  onAllowed,
  onAnswered,
  onRestarted,
  onWay,
}: {
  block: HeldBlock;
  /** The asks the registry lists for this block (`sandboxBlocks.asksOf`): what it asks from. */
  asks?: readonly Shown[];
  /** How many other blocks this chat holds behind this one. */
  more: number;
  onDismiss: () => void;
  /** Something was allowed: the chat is owed a restart once its turn has ended. */
  onAllowed: () => void;
  /** Something was allowed here, taken live or not: what it said stands though its ask goes. */
  onAnswered?: () => void;
  /** The chat started again in its place, without the sandbox. */
  onRestarted: (chat: OpenChat) => void;
  /**
   * **An answer was pressed** (#1700, 2026-10-10 21:09 item 2): its words, once for each of the
   * registry's asks it answers, so the pane can list the answer once that ask goes
   * (`useAnsweredWhenGone`). Said by an Allow and by Keep blocked as they send, never by a press
   * that sends nothing, nor by Report, Restart, Retry or Start without the sandbox, which come
   * after an answer rather than being one.
   */
  onWay?: (words: string, ask: Shown) => void;
}) {
  const id = useId();
  /** The Report's draft once it is open, what filing it answered, and any refusal. */
  const [draft, setDraft] = useState<BlockReport>();
  const [filed, setFiled] = useState<string>();
  const [said, setSaid] = useState<string>();
  const [filing, setFiling] = useState(false);
  /** Whether the person took up a block the chat only read back (#1884). */
  const [takenUp, setTakenUp] = useState(false);
  const [taking, setTaking] = useState(false);
  const cause = `sandbox-blocked:${block.session}:${block.operation}:${block.kind}:${block.ours ? "ours" : "chat"}`;
  const behind =
    more > 0 ? ` ${more} more ${more === 1 ? "block" : "blocks"} behind this one.` : "";

  if (!block.ours && typeof block.quiet === "string" && !takenUp) {
    /** What the core holds for it, which taking it up makes an ask: nothing for a block no
     *  answer names, whose own Notice is drawn with no word to the core. */
    const held =
      block.choose === "socket" || block.choose === "service"
        ? [{ what: block.choose, target: "" }]
        : isAnAsk(block)
          ? eachHost(block).map((one) => ({
              what: block.offer as "host" | "write" | "socket" | "service" | "port",
              target: one.target ?? "",
            }))
          : [];
    const takeUp = () => {
      if (taking) return;
      if (held.length === 0) {
        setTakenUp(true);
        return;
      }
      setTaking(true);
      setSaid(undefined);
      void Promise.all(
        held.map((one) =>
          commands
            .takeUpSandboxBlock(block.plane, block.session, {
              operation: block.operation,
              kind: block.kind,
              ...one,
            })
            .then((done) => (done.status === "error" ? done.error : undefined))
            .catch((err: unknown) => `purlis could not show it: ${String(err)}`),
        ),
      )
        .then((answers) => {
          const refused = answers.find((one) => one !== undefined);
          if (refused !== undefined) setSaid(refused);
          else {
            setTakenUp(true);
            asksMoved(block.plane);
          }
        })
        .finally(() => setTaking(false));
    };
    const line = (
      <>
        This chat read a sandbox refusal back: {block.said}. {block.quiet}
        {said !== undefined && ` ${said}`}
        {behind}
      </>
    );
    // Taking one up makes it ask, which is the window's alone to do; a block nothing holds
    // only opens here.
    return held.length === 0 || inTheWindow(commands, "takeUpSandboxBlock") ? (
      <Notice
        cause={`${cause}:read-back`}
        at="pane"
        tone="news"
        label="Sandbox block"
        fixes={[{ label: "Show what you can do", onPress: takeUp, busy: taking }]}
        onDismiss={onDismiss}
      >
        {line}
      </Notice>
    ) : (
      <Notice
        cause={`${cause}:read-back`}
        at="pane"
        tone="news"
        label="Sandbox block"
        onDismiss={onDismiss}
      >
        {line}
      </Notice>
    );
  }

  if (!block.ours)
    return (
      <AllowNotice
        block={block}
        asks={asks}
        cause={cause}
        behind={behind}
        onDismiss={onDismiss}
        onAllowed={onAllowed}
        onAnswered={onAnswered}
        onRestarted={onRestarted}
        onWay={onWay}
      />
    );

  const report = () => {
    if (draft !== undefined) {
      setDraft(undefined);
      return;
    }
    setSaid(undefined);
    void commands
      .sandboxBlockReport(block.operation, block.kind, block.harness)
      .then((made) => {
        if (made.status === "error") setSaid(made.error);
        else setDraft(made.data);
      })
      .catch((err: unknown) => setSaid(String(err)));
  };
  const file = (shown: BlockReport) => {
    setFiling(true);
    setSaid(undefined);
    void commands
      .fileSandboxBlockReport(block.operation, block.kind, block.harness, shown.digest)
      .then((done) => {
        if (done.status === "error") setSaid(done.error);
        else {
          setFiled(done.data);
          setDraft(undefined);
        }
      })
      .catch((err: unknown) => setSaid(String(err)))
      .finally(() => setFiling(false));
  };

  const under =
    draft === undefined ? undefined : (
      <Card className="block-report" id={id}>
        <p>
          Nothing is sent until you press File report. It would be filed on {draft.repository} as
          you, with purlis&apos;s sign-in to GitHub.
        </p>
        <pre className="block-report-draft" aria-label="Report draft">
          {`${draft.title}\n\n${draft.body}`}
        </pre>
        <ButtonRow size="sm" className="block-report-actions">
          {inTheWindow(commands, "fileSandboxBlockReport") && (
            <Button size="sm" variant="primary" isDisabled={filing} onPress={() => file(draft)}>
              File report
            </Button>
          )}
          <Button size="sm" onPress={() => setDraft(undefined)}>
            Cancel
          </Button>
        </ButtonRow>
      </Card>
    );

  return (
    <Notice
      cause={cause}
      at="pane"
      tone="trouble"
      label="Sandbox block"
      fixes={
        filed === undefined
          ? [{ label: "Report…", onPress: report, opens: { id, open: draft !== undefined } }]
          : undefined
      }
      onDismiss={onDismiss}
      under={under}
    >
      The sandbox blocked {block.said}. It was purlis&apos;s own operation, so it is a purlis bug.
      {filed !== undefined && ` Reported: ${filed}.`}
      {said !== undefined && ` ${said}`}
      {behind}
    </Notice>
  );
}

const LEVELS: readonly GrantLevel[] = ["once", "chat", "you", "machine", "project"];

const isLevel = (id: string): id is GrantLevel => (LEVELS as readonly string[]).includes(id);

/** What a port field pre-filled from the command's own output says (qw384): the sandbox did not
 *  name it, so the person checks it before allowing. */
const PORT_FROM_OUTPUT = "This port is from the command's output: check it.";

/** What a block names, drawn apart from purlis's words and safely (`inertly`). */
const Target = ({ of }: { of: string }) => (
  <code className="block-allow-target">{inertly(of)}</code>
);

/** What a hold that ran out says (#1709). */
export const HOLD_RAN_OUT =
  "Nobody answered within a minute, so the connection was refused. An Allow now tells the chat to run it again.";

/**
 * **The line Tell the chat to retry puts in the chat's prompt** (#1709, D-1709-7): fixed words,
 * naming nothing the chat said. The fixed retry line purlis tells the chat on its next turn says
 * which hosts; this only gives an idle chat that turn, when the person sends it.
 */
export const RETRY_LINE = "The connection you waited on is allowed now. Run that command again.";

/**
 * **" while running a command with vault <name>"** (#1353): said right after what was blocked,
 * for a host a brokered command's sandbox refused; nothing for a block of the chat's own command.
 * The name is drawn as the core sent it, already inert.
 */
function RanWith({ vault }: { vault: string | null | undefined }) {
  if (vault === null || vault === undefined) return null;
  return (
    <>
      {" "}
      while running a command with vault <code className="block-vault">{vault}</code>
    </>
  );
}

/**
 * **What Use <context> gives the chat** (#2097 review), said before any press: the copy holds
 * the context's credentials, as the person's kubeconfig does, and the chat can read it.
 */
export const KUBECTL_COPY_SAID =
  "This chat gets its own copy of the context you choose, its credentials included: this chat can read them. Your own kubeconfig is not changed.";

/** The harness word a Claude Code chat's blocks carry. */
const CLAUDE_CODE = "claude";

/**
 * **Point kubectl at it** (qw384b): after a Claude Code chat is allowed a service on this Mac,
 * its kubectl still connects to `127.0.0.1` or `localhost` itself, which never reaches the
 * chat's proxy. On the person's press, the contexts of their kubeconfig are listed, and one on
 * this Mac is copied for this chat alone, its server named as the proxy carries it. Nothing is
 * read before the press; a context elsewhere is listed with why, never pressable; and the
 * person's own kubeconfig is never written. The chat restarts to take it, as after an Allow.
 */
function PointKubectl({ block, onPointed }: { block: HeldBlock; onPointed: () => void }) {
  const id = useId();
  const [contexts, setContexts] = useState<readonly KubeContext[]>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [pointed, setPointed] = useState(false);
  if (!inTheWindow(commands, "kubeContexts") || !inTheWindow(commands, "pointKubectl")) return null;
  const list = () => {
    setBusy(true);
    void commands
      .kubeContexts()
      .then((answer) => {
        if (answer.status === "error") setSaid(answer.error);
        else setContexts(answer.data);
      })
      .finally(() => setBusy(false));
  };
  const point = (context: string) => {
    setBusy(true);
    void commands
      .pointKubectl(block.plane, block.session, context)
      .then((answer) => {
        if (answer.status === "error") setSaid(answer.error);
        else {
          setSaid(answer.data);
          setPointed(true);
          sandboxCommandReturned();
          onPointed();
        }
      })
      .finally(() => setBusy(false));
  };
  return (
    <Card className="block-kubectl" id={id}>
      {pointed ? null : contexts === undefined ? (
        <ButtonRow size="sm">
          <Button size="sm" isDisabled={busy} onPress={list}>
            Point kubectl at it…
          </Button>
        </ButtonRow>
      ) : contexts.length === 0 ? (
        <p>Your kubeconfig holds no context.</p>
      ) : (
        <>
          <p>{KUBECTL_COPY_SAID}</p>
          <ul className="block-kubectl-contexts">
            {contexts.map((context) => (
              <li key={context.name}>
                {context.refused === null ? (
                  <Button size="sm" isDisabled={busy} onPress={() => point(context.name)}>
                    Use {inertly(context.name)}
                  </Button>
                ) : (
                  <>
                    <code>{inertly(context.name)}</code>: {inertly(context.refused)}
                  </>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </Card>
  );
}

/** The time now: when a host joined, read once in the render that draws it, or when pressed. */
const joinedAt = () => performance.now();

/**
 * **Allow, Always allow or Keep blocked** for a block of the chat's own work (#1342). Nothing is
 * allowed until a press, and what a press allows is on screen, whole, before it.
 */
function AllowNotice({
  block,
  asks,
  cause,
  behind,
  onDismiss,
  onAllowed,
  onAnswered,
  onRestarted,
  onWay,
}: {
  block: HeldBlock;
  asks: readonly Shown[];
  cause: string;
  behind: string;
  onDismiss: () => void;
  onAllowed: () => void;
  onAnswered?: () => void;
  onRestarted: (chat: OpenChat) => void;
  onWay?: (words: string, ask: Shown) => void;
}) {
  const id = useId();
  /** A port only the command's own output named (qw384): pre-filled for the person to check,
   *  never taken as the refusal's own word. */
  const suggested = block.target === null ? (block.suggested ?? undefined) : undefined;
  const [typed, setTyped] = useState(block.target ?? suggested ?? "");
  const [alwaysOpen, setAlways] = useState(false);
  const [allowed, setAllowed] = useState<Allowed>();
  /** The hosts an Allow on this Notice allowed (#1637). */
  const [allowedHosts, setAllowedHosts] = useState<readonly string[]>([]);
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** Why the project is read-only, when it is: an Allow for everyone in the project writes its
   *  file, so it is off with the sentence (#826). */
  const readOnly = useReadOnlySaid();
  /** An Allow taken live after the hold had run out: the chat is told to retry on its next
   *  turn, which an idle chat has only once the person sends it something (#1709). */
  const [owedRetry, setOwedRetry] = useState(false);
  /** When Tell the chat to retry appeared, where the Allow just pressed was: a second press of
   *  that Allow lands on it, so for {@link SETTLE_MS} after, it does nothing and says why. */
  const [owedSince, setOwedSince] = useState(-Infinity);
  /** The hosts the chat's proxy refused past the ones it names (#1353): said, never offered. */
  const moreRefused = useRefusedMore(block.plane, block.session);
  const what: GrantWhat =
    block.offer === "host" ||
    block.offer === "socket" ||
    block.offer === "service" ||
    block.offer === "port"
      ? block.offer
      : "write";
  const target = block.target ?? typed.trim();
  /** A folder, a socket, a service or a port reaches the chat only by restarting it, at its next pause
   *  (#1986): what that ends is said before the Allow. A host the chat's proxy may take live
   *  restarts nothing, so it says none of this. */
  const restarts = what !== "host";
  /** The hosts this Notice lists that no Allow on it has allowed yet and the registry still
   *  lists, oldest first: a host answered anywhere else is no longer asked about here. */
  const hosts = hostsOf(block);
  const listedHost = (host: string) =>
    asks.some(
      (ask) =>
        ask.answer.via === "sandbox-block" &&
        matched("host", ask.answer.shown.target) === matched("host", host),
    );
  const asking = hosts.filter((host) => !allowedHosts.includes(host) && listedHost(host));
  /**
   * **When the hosts it asks about last changed** (#1637): a host that joins the Notice changes
   * what one Allow grants, so for {@link SETTLE_MS} after, a press does nothing and says why, as
   * the question for several tasks does. Never at its first drawing: nothing joined it yet.
   */
  const listing = asking.join(" ");
  const [joined, setJoined] = useState({ listing, at: -Infinity });
  if (joined.listing !== listing) {
    const grew = asking.some((host) => !joined.listing.split(" ").includes(host));
    setJoined({ listing, at: grew ? joinedAt() : joined.at });
  }
  /**
   * Whether a press now is too soon after a host joined what it would allow: as this Notice saw
   * it join, or as the window's store held it, so a Notice drawn again keeps the guard.
   */
  const tooSoon = () => {
    if (joinedAt() - Math.max(joined.at, block.joined ?? -Infinity) < SETTLE_MS) {
      setSaid(
        "A host joined this Notice just now, so nothing was allowed. Read the hosts it lists and " +
          "answer again.",
      );
      return true;
    }
    return false;
  };
  /**
   * **The answers, as the registry's ask offers them** (#1700): its order and its words, at the
   * levels policy leaves open on both (#1343). A host: this project on this machine first
   * (#1666, N-3), then only this chat, then everyone in the project. A folder: this chat, then
   * this machine; never the project.
   */
  const offered: readonly Offered[] = asks[0]?.options ?? [];
  const open = offered.filter(
    (option): option is Offered & { id: GrantLevel } =>
      option.allows && isLevel(option.id) && block.levels.includes(option.id),
  );
  const main = open[0];
  /** Every project on this machine (R2): drawn beside the main button, never under the menu,
   *  where a socket's or a service's ask offers it and it is not the main button itself. */
  const everywhere = open.slice(1).find((option) => option.id === "machine");
  const others = open.slice(1).filter((option) => option !== everywhere);
  const keepSaid = offered.find((option) => option.id === "keep")?.label;
  /** Whether the chat's proxy holds the connection now, as the registry says (#1709). */
  const held = asks.some((ask) => typeof ask.held_until === "number");
  /** Held when it arrived, and held no more: its hold ran out (#1709). */
  const ranOut = block.held && !held;

  /** Says `words` with each ask a press on `targets` answers (`onWay`): the asks of the hosts
   *  it sends, or, for a Notice that lists no host, every ask it was drawn from. */
  const sayWay = (words: string, targets: readonly string[]) => {
    for (const ask of asks) {
      const of = ask.answer.via === "sandbox-block" ? ask.answer.shown.target : undefined;
      if (
        hosts.length === 0 ||
        (of !== undefined && targets.some((one) => matched("host", one) === matched("host", of)))
      )
        onWay?.(words, ask);
    }
  };
  const allow = (level: GrantLevel, words: string) => {
    if (busy) return;
    if (target === "") return;
    if (tooSoon()) return;
    setBusy(true);
    setSaid(undefined);
    // Each host the Notice lists and has not allowed, or the one thing it names (#1637): what
    // is on screen when pressed, never a host that joins it while the answer is on its way.
    const targets = hosts.length > 0 ? asking : [target];
    sayWay(words, targets);
    const hadRunOut = ranOut;
    void (async () => {
      const done: string[] = [];
      const refused: string[] = [];
      let last: Allowed | undefined;
      /** Whether every host allowed was taken live: then nothing restarts for them (#1666). */
      let live = true;
      for (const one of targets) {
        // The block it showed, so the core answers only that one, and only while the chat is
        // still held on it (#1538).
        const named = targets.length > 1 ? `${one}: ` : "";
        try {
          const answer = await commands.allowSandboxBlock(
            block.plane,
            block.session,
            { operation: block.operation, kind: block.kind, what, target: one },
            level,
          );
          if (answer.status === "error") refused.push(`${named}${answer.error}`);
          else {
            done.push(one);
            last = answer.data;
            live = live && answer.data.live === true;
          }
        } catch (err: unknown) {
          refused.push(`${named}purlis could not allow it: ${String(err)}`);
        }
      }
      if (last !== undefined) {
        setAllowedHosts((was) => [...was, ...done]);
        setAllowed(last);
        setOwedRetry(hadRunOut && live);
        setOwedSince(joinedAt());
        setAlways(false);
        onAnswered?.();
        // One restart takes every host allowed, unless the chat's proxy took them live.
        if (!live) onAllowed();
      }
      if (refused.length > 0) setSaid(refused.join(" "));
    })().finally(() => {
      setBusy(false);
      // Allowed for every chat, the other chats keep the sandbox they started with: the
      // Notice for chats left behind asks again (#1428).
      sandboxCommandReturned();
      // The Inbox lists the same hosts (#1692): answered here, they go there too.
      asksMoved(block.plane);
    });
  };
  /** Keep blocked (#1666, #1987): the core refuses what the proxy holds on a host, and tells
   *  the chat, whatever the block is. */
  const keepBlocked = () => {
    if (busy) return;
    const targets = hosts.length > 0 ? asking : block.target !== null ? [block.target] : [];
    if (keepSaid !== undefined) sayWay(keepSaid, targets);
    if (targets.length === 0) {
      onDismiss();
      return;
    }
    // Every kind is kept blocked by the core, which tells the chat in one line (#1987); a
    // block answered already is a quiet no-op there, and the Notice goes.
    setBusy(true);
    setSaid(undefined);
    void Promise.all(
      targets.map((one) =>
        commands
          .keepSandboxBlock(
            block.plane,
            block.session,
            {
              operation: block.operation,
              kind: block.kind,
              what,
              target: one,
            },
            null,
          )
          .then((done) => (done.status === "error" ? done.error : undefined))
          .catch((err: unknown) => `purlis could not keep it blocked: ${String(err)}`),
      ),
    )
      .then((answers) => {
        const refused = answers.filter((one): one is string => one !== undefined);
        // Said, never swallowed: the Notice stays up with why (docs/ui-copy.md).
        if (refused.length > 0) setSaid(refused.join(" "));
        else onDismiss();
      })
      .finally(() => setBusy(false));
  };
  /**
   * **Tell the chat to retry** (#1709, D-1709-7): purlis never types into an idle chat by
   * itself. On the person's press, the fixed {@link RETRY_LINE} goes into the chat's prompt as
   * one bracketed paste with no Return, as a curation prompt does (ADR 0061), and the person
   * sends it.
   */
  const tellToRetry = () => {
    if (busy) return;
    if (joinedAt() - owedSince < SETTLE_MS) {
      setSaid(
        "This button appeared just now, so nothing was put in the chat's prompt. Press it again.",
      );
      return;
    }
    setBusy(true);
    setSaid(undefined);
    void commands
      .sendInput(block.plane, block.session, `\x1b[200~${RETRY_LINE}\x1b[201~`)
      .then((done) => {
        if (done.status === "error") setSaid(done.error);
        else {
          setOwedRetry(false);
          setSaid("The line is in the chat's prompt: press Return there to send it.");
        }
      })
      .catch((err: unknown) => setSaid(`purlis could not put the line there: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  /** Restart this chat, for a host allowed already (#1666's fold-in). */
  const restartThis = () => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .askChatRestart(block.plane, block.session)
      .then((done) => {
        if (done.status === "error") setSaid(done.error);
        else {
          onAllowed();
          onDismiss();
        }
      })
      .catch((err: unknown) => setSaid(`purlis could not restart the chat: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  /** Whether the form a socket or a service is chosen in is open (#1884). */
  const [choosing, setChoosing] = useState(false);
  /** Choose the socket…, where the block lets the person choose and this page can carry it. */
  const choose: NoticeAction[] =
    (block.choose === "socket" || block.choose === "service") && mayChoose()
      ? [
          {
            label: chooseSaid(block),
            onPress: () => setChoosing((was) => !was),
            opens: { id, open: choosing },
          },
        ]
      : [];
  const [chooseIt] = choose;
  const chooseUnder =
    choosing && chooseIt !== undefined ? <SandboxChoose block={block} id={id} /> : undefined;
  const withoutSandbox = () => {
    setBusy(true);
    setSaid(undefined);
    void commands
      .restartChatWithoutSandbox(block.plane, block.session, 80, 24)
      .then((done) => {
        if (done.status === "error") setSaid(done.error);
        else onRestarted(done.data);
      })
      .catch((err: unknown) => setSaid(`purlis could not start the chat again: ${String(err)}`))
      .finally(() => setBusy(false));
  };

  if (block.offer === "allowed")
    return (
      <Notice
        cause={cause}
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[{ label: "Restart this chat", onPress: restartThis, busy }]}
        onDismiss={onDismiss}
      >
        The sandbox blocked {block.said}
        <RanWith vault={block.vault} />
        {block.target !== null && (
          <>
            : <code className="block-allow-target">{inertly(block.target)}</code>
          </>
        )}
        . {block.route}
        {said !== undefined && ` ${said}`}
        {behind}
      </Notice>
    );
  if (block.offer === "brokered")
    return (
      <Notice cause={cause} at="pane" tone="trouble" label="Sandbox block" onDismiss={onDismiss}>
        The sandbox blocked {block.said}. purlis never allows that to a chat. {block.route}
        {behind}
      </Notice>
    );
  if (block.offer === "policy") {
    const says = (
      <>
        The sandbox blocked {block.said}
        <RanWith vault={block.vault} />
        {block.target !== null && (
          <>
            {block.kind === "host" ? ": " : " in "}
            <code className="block-allow-target">{inertly(block.target)}</code>
          </>
        )}
        . {block.route}
        {behind}
      </>
    );
    // Policy forbids starting without the sandbox, and a socket may still be chosen (#1884).
    return chooseIt === undefined ? (
      <Notice cause={cause} at="pane" tone="trouble" label="Sandbox block" onDismiss={onDismiss}>
        {says}
      </Notice>
    ) : (
      <Notice
        cause={cause}
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[chooseIt]}
        onDismiss={onDismiss}
        under={chooseUnder}
      >
        {says}
      </Notice>
    );
  }
  if (block.offer === "unsandboxed") {
    const startWithout: NoticeAction = {
      label: "Start without the sandbox for this chat",
      onPress: () => {
        if (!busy) withoutSandbox();
      },
    };
    return (
      <Notice
        cause={cause}
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={chooseIt === undefined ? [startWithout] : [chooseIt, startWithout]}
        onDismiss={onDismiss}
        under={chooseUnder}
      >
        The sandbox blocked {block.said}
        <RanWith vault={block.vault} />
        {block.target !== null && (
          <>
            {block.kind === "host" ? ": " : " in "}
            <code className="block-allow-target">{inertly(block.target)}</code>
          </>
        )}
        . {block.route} Only you can choose to start this chat again without the sandbox: it
        restarts now, even mid-turn, on the same conversation, and nothing it runs is confined until
        it next starts. Anything allowed for it that it has not yet taken is dropped.
        {said !== undefined && ` ${said}`}
        {behind}
      </Notice>
    );
  }
  if (
    block.offer !== "host" &&
    block.offer !== "write" &&
    block.offer !== "socket" &&
    block.offer !== "service" &&
    block.offer !== "port"
  )
    return (
      <Notice cause={cause} at="pane" tone="trouble" label="Sandbox block" onDismiss={onDismiss}>
        The sandbox blocked {block.said}.{behind}
      </Notice>
    );
  /** The hosts said whole, each apart from purlis's sentence. */
  const drawn = (of: readonly string[]) =>
    of.map((host, at) => (
      <span key={host}>
        {at > 0 && (at === of.length - 1 ? " and " : ", ")}
        <code className="block-allow-target">{inertly(host)}</code>
      </span>
    ));
  if (allowed !== undefined && asking.length === 0) {
    const allowedSays = (
      <>
        {hosts.length > 0 ? (
          <>{drawn(allowedHosts)}: </>
        ) : (
          block.target !== null && (
            <>
              <code className="block-allow-target">{inertly(block.target)}</code>:{" "}
            </>
          )
        )}
        {allowed.said}
        {said !== undefined && ` ${said}`}
        {what === "port" && block.harness === CLAUDE_CODE && (
          <PointKubectl block={block} onPointed={onAllowed} />
        )}
      </>
    );
    return owedRetry ? (
      <Notice
        cause={`${cause}:allowed`}
        at="pane"
        tone="news"
        label="Sandbox block"
        onDismiss={onDismiss}
        fixes={[{ label: "Tell the chat to retry", onPress: tellToRetry, busy }]}
      >
        {allowedSays}
      </Notice>
    ) : (
      <Notice
        cause={`${cause}:allowed`}
        at="pane"
        tone="news"
        label="Sandbox block"
        onDismiss={onDismiss}
      >
        {allowedSays}
      </Notice>
    );
  }

  // A block that named nothing asks for it in a form row under the line (#626): the set's
  // Field, named and described by its row, in the Notice's own place for what it opens.
  const named =
    block.target === null ? (
      <SettingRow
        label={
          what === "host" ? "Host to allow" : what === "port" ? "Port to allow" : "Folder to allow"
        }
        help={
          what === "host"
            ? "An Allow lets the chat reach the host you type here."
            : what === "port"
              ? suggested !== undefined && typed.trim() === suggested
                ? `${PORT_FROM_OUTPUT} An Allow lets the chat connect to whatever listens on this Mac at that port, as you.`
                : "An Allow lets the chat connect to whatever listens on this Mac at the port you type here, as you."
              : "An Allow lets the chat write in the folder you type here and everything in it."
        }
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={typed}
            onChange={setTyped}
            placeholder={what === "host" ? "api.example.com" : what === "port" ? "6443" : undefined}
          />
        )}
      />
    ) : (
      <p>
        {what === "host" ? (
          <>Allow reaching {hosts.length > 0 ? drawn(asking) : <Target of={block.target} />}</>
        ) : what === "port" ? (
          // qw384: one port, named whole, with what allowing it means, in purlis's words.
          <>
            Allow connecting to the service on this Mac at port <Target of={block.target} />.{" "}
            {block.route}
          </>
        ) : what === "socket" || what === "service" ? (
          // D-sock-6: named whole, with what allowing it means, in purlis's words.
          <>
            Allow {what === "socket" ? "connecting to" : "reaching the service"}{" "}
            <Target of={block.target} />. {block.route}
          </>
        ) : (
          <>
            Allow writing {hosts.length > 0 ? drawn(asking) : <Target of={block.target} />} and
            everything in it
          </>
        )}
      </p>
    );
  const under = (
    <div className="block-allow" id={id}>
      {named}
      {alwaysOpen && (
        <ButtonRow size="sm" className="block-allow-actions">
          {others.map((option) => (
            <Button
              size="sm"
              variant="grants"
              key={option.id}
              isDisabled={
                busy || target === "" || (option.id === "project" && readOnly !== undefined)
              }
              tooltip={option.id === "project" ? readOnly : undefined}
              onPress={() => allow(option.id, option.label)}
            >
              {option.label}
            </Button>
          ))}
        </ButtonRow>
      )}
    </div>
  );
  // Only the levels policy leaves open (#1343); Keep blocked always.
  const keep: NoticeAction | undefined =
    keepSaid === undefined ? undefined : { label: keepSaid, onPress: keepBlocked };
  const allows: NoticeAction[] = [
    ...(main !== undefined
      ? [
          {
            label: main.label,
            onPress: () => allow(main.id, main.label),
            off: main.id === "project" ? readOnly : undefined,
            allows: true,
          },
        ]
      : []),
    ...(everywhere !== undefined
      ? [
          {
            label: everywhere.label,
            onPress: () => allow(everywhere.id, everywhere.label),
            allows: true,
          },
        ]
      : []),
    ...(others.length > 0
      ? [
          {
            label: what === "write" ? "Always allow…" : "Other scopes…",
            onPress: () => setAlways((was) => !was),
            opens: { id, open: alwaysOpen },
          },
        ]
      : []),
  ];
  const [first, ...rest] = [...allows, ...(keep === undefined ? [] : [keep])];
  // Answered anywhere, or never held: the registry lists no ask for it, so it asks nothing.
  if (first === undefined) return null;
  const fixes: readonly [NoticeAction, ...NoticeAction[]] = [first, ...rest];
  return (
    <Notice
      cause={cause}
      at="pane"
      tone="trouble"
      label="Sandbox block"
      fixes={fixes}
      under={under}
    >
      {held ? (
        <>
          The chat's command is waiting on {block.said}
          <RanWith vault={block.vault} />: purlis holds the connection while you answer, and an
          Allow lets the same command carry on. If nobody answers within a minute it is refused, and
          an Allow after that tells the chat to run it again.
        </>
      ) : ranOut ? (
        <>
          The chat's command waited on {block.said}
          <RanWith vault={block.vault} />. {HOLD_RAN_OUT}
        </>
      ) : (
        <>
          The sandbox blocked {block.said}
          <RanWith vault={block.vault} />.
        </>
      )}
      {asking.length > 1 &&
        (held
          ? ` ${asking.length} hosts are asked about. One Allow allows each of them.`
          : ` ${asking.length} hosts were refused. One Allow allows each of them, and the chat restarts once.`)}
      {what === "host" && moreRefused > 0 && ` ${moreRefusedSaid(moreRefused)}`}
      {block.ruled !== null && block.ruled !== undefined && ` ${block.ruled}`}
      {restarts && (
        <>
          {" "}
          <RestartEndsLine plane={block.plane} session={block.session} />
        </>
      )}
      {allowedHosts.length > 0 && <> Allowed already: {drawn(allowedHosts)}.</>}
      {said !== undefined && ` ${said}`}
      {behind}
    </Notice>
  );
}
