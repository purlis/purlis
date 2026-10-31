// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type RouterGradeStep,
  type SkillPinner,
  type SkillRouterOffer,
  type StudioRouted,
  type StudioRouter,
} from "./bindings";
import { counted } from "./dispatches";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
import { Fact, Facts } from "./ui/card/Facts";
import { Radio, RadioGroup } from "./ui/choice/Radio";
import { Switch } from "./ui/choice/Switch";
import { Dialog } from "./ui/dialog/Dialog";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRow, type Selection } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewPageHeader } from "./ui/panel/ViewFrame";
import { routedChatId } from "./skillStudio";
import { capitalised } from "./saving";

/** The word a choice of the built-in router is written as (`purlis_core::skills::router`). */
export const BUILT_IN_ROUTER = "built-in";

/**
 * **What the budget counts, and what it does not** (#3324, ADR 0102 as amended 2026-10-31): the
 * skills purlis hands a chat. A program's own catalogue is its own, listed against its own limit:
 * purlis never trims what it does not own, so it is shown beside the budget, never in it.
 */
export const BUDGET_COUNTS =
  "It counts the skills purlis hands a chat: the built-in ones, the project's, and those of the repo the chat works in. A program's own skills, in your folders and its plugins, are not counted: the program lists them against its own limit, and the Skills view shows them beside a chat's budget.";

/**
 * **What the person turns on for a router that reads prompts** (ADR 0102 as amended 2026-10-31):
 * always on screen beside the switch, on or off.
 */
export const READS_EVERY_PROMPT =
  "On, this router reads every prompt of every chat it routes on this machine, to point the chat at skills its list leaves out. It runs confined and has 100 ms: past that the prompt goes on with no pointer, and the routing record says it fell back. purlis writes no prompt down. Turning it on reaches chats started after; turning it off stops at the next prompt.";

/** What Change… sends for a persona's choice taken out: its chats take their workspace's. */
const THE_WORKSPACES = "";

/** What is chosen now where what is written is no router's name: no option is it. */
const UNREADABLE = "\u0000";

/** Whose chats a router is chosen for, as a sentence names them. */
function whoseSaid(by: SkillPinner): string {
  return by.at === "workspace" ? `chats in ${by.name}` : `chats as ${by.name}`;
}

/** A router as a sentence names it: the built-in router, the router relevance. */
function routerSaid(router: string): string {
  return router === BUILT_IN_ROUTER ? "the built-in router" : `the router ${router}`;
}

/** What a chat's start row says under its number: why it fell back, or what it hid. */
function lineOf(chat: StudioRouted): string {
  if (chat.routing.fell_back) return chat.routing.said;
  return chat.hidden.length === 0
    ? "It hid nothing at the chat's start."
    : `It hid ${counted(chat.hidden.length, "skill", "skills")} at the chat's start.`;
}

/** A chat's start's one mark: its timing, or that it fell back on the built-in router. */
function markOf(chat: StudioRouted) {
  return chat.routing.fell_back ? (
    <Badge variant="waiting">Fell back to the built-in router</Badge>
  ) : (
    <Badge>{`${chat.routing.ms} ms`}</Badge>
  );
}

/**
 * **The Routing page's facts** (#3692, S9, `prototype/studio-secondary-pages` variant A): the
 * router these chats are routed by, with Change… (a dialog), the budget past which it decides
 * what a chat's list shows, and an extension's newest grade with its Scorecard (#3617) and Grade
 * the router…, which opens its plan beside the list (SK-15e, #4222). At the project's root, which chooses no router, Change… is out of reach and says why.
 */
export function RoutingFacts({
  plane,
  router,
  by,
  persona,
  busy,
  onChoose,
  onReadsPrompts,
  gradeRunning,
  onGradeRunning,
  onGradePlan,
  onScorecard,
}: {
  plane: string;
  router: StudioRouter;
  /** Whose router a change writes: the scope's workspace or persona, none at the root. */
  by: SkillPinner | null;
  persona: string | null;
  busy: boolean;
  /** Chooses `router` for `by`'s chats (null takes the choice out), and says `said` once it is. */
  onChoose: (by: SkillPinner, router: string | null, said: string) => void;
  /** Turns the chosen router's reading of prompts on or off, and says `said` once it is. */
  onReadsPrompts: (router: string, on: boolean, said: string) => void;
  /** The router's grade running in this scope, and the way to its page and its Stop (#4222). */
  gradeRunning?: RouterGradeStep;
  onGradeRunning: (grade: string) => void;
  /** Opens the router's grade plan beside the list, a review page (#4222). */
  onGradePlan: () => void;
  /** Opens the scorecard of the router's grade `grade` beside the list (#3617). */
  onScorecard: (grade: string) => void;
}) {
  const [changing, setChanging] = useState(false);
  const { budget } = router;
  const extension = router.extension;
  // Read when the page shows an extension's router, and again each time the studio is read, as
  // Change…'s offers are: the studio's own read asks no extension's manifest.
  const [offers, setOffers] = useState<readonly SkillRouterOffer[]>([]);
  useEffect(() => {
    if (extension === null) return;
    let live = true;
    void commands
      .skillRouters(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setOffers(answer.data);
      })
      .catch(() => live && setOffers([]));
    return () => {
      live = false;
    };
  }, [plane, extension, router]);
  const offer = offers.find((one) => one.id === extension);
  const shown = router.problem ? (
    <>
      <Badge variant="waiting">Not a router</Badge> {router.said}
    </>
  ) : router.extension !== null ? (
    <>
      <code>{router.extension}</code> <Badge>Extension</Badge>
    </>
  ) : (
    "Built-in"
  );
  return (
    <div className="skill-studio-page" data-testid="skill-routing-facts">
      <Facts aria-label="How these chats are routed">
        <Fact label="Router">
          <span className="skill-routing-router">
            <span data-testid="skill-routing-router">{shown}</span>{" "}
            <Button
              size="sm"
              isDisabled={by === null || busy}
              tooltip={
                by === null
                  ? "A router is chosen for a workspace or a persona: pick one in Whose chats."
                  : undefined
              }
              onPress={() => setChanging(true)}
            >
              Change…
            </Button>
          </span>
          {persona !== null && router.written === null && (
            <span className="honest">
              {` In a workspace, chats as ${persona} take that workspace's router.`}
            </span>
          )}
        </Fact>
        {extension !== null && offer?.reads_prompts === true && (
          <Fact label="Prompts">
            <Switch
              isSelected={offer.prompts_on}
              isDisabled={busy}
              onChange={(on) =>
                onReadsPrompts(
                  extension,
                  on,
                  on
                    ? `The router ${extension} now reads every prompt of chats started after this.`
                    : `The router ${extension} reads no further prompt.`,
                )
              }
            >
              Let it read prompts
            </Switch>
            <span className="honest" data-testid="skill-routing-reads-every-prompt">
              {` ${READS_EVERY_PROMPT}`}
            </span>
          </Fact>
        )}
        <Fact label="Preview">
          <span data-testid="skill-routing-preview">{router.preview}</span>
        </Fact>
        <Fact label="Budget">
          {`${counted(budget.skills, "skill", "skills")}, ${budget.chars.toLocaleString("en-US")} characters of descriptions, on every program.`}
          <span className="honest" data-testid="skill-routing-budget-counts">
            {` ${BUDGET_COUNTS}`}
          </span>
        </Fact>
        {router.extension !== null && (
          <Fact label="Grade">
            <span data-testid="skill-studio-router-grade">
              {router.grade ??
                "Not graded on this library yet: a grade runs the library's trigger tests with it and with the built-in router."}
            </span>{" "}
            {router.graded !== null && (
              <Button size="sm" onPress={() => onScorecard(router.graded ?? "")}>
                Scorecard
              </Button>
            )}{" "}
            <Button size="sm" onPress={onGradePlan}>
              Grade the router…
            </Button>
          </Fact>
        )}
        {gradeRunning !== undefined && (
          // Whichever router is chosen now: the grade spends until it is stopped.
          <Fact label="Grading">
            <span data-testid="skill-routing-grading">
              {`The router ${gradeRunning.router}: ${gradeRunning.ran} of ${counted(gradeRunning.of, "chat", "chats")} done.`}
            </span>{" "}
            <Button size="sm" onPress={() => onGradeRunning(gradeRunning.grade)}>
              Show the grade
            </Button>
          </Fact>
        )}
      </Facts>
      {changing && by !== null && (
        <RouterChange
          plane={plane}
          by={by}
          written={router.written}
          unreadable={router.problem && router.written === null}
          onClose={() => setChanging(false)}
          onChoose={(chosen, said) => {
            setChanging(false);
            onChoose(by, chosen, said);
          }}
        />
      )}
    </div>
  );
}

/**
 * **Change…: the router a workspace's or a persona's chats are routed by** (#3692): the built-in
 * router or each router extension approved here and on in the project, read as the dialog
 * opens; for a persona, its workspace's router too. It says what changes and for whom before
 * the person confirms; chats open now keep the router their start asked.
 */
export function RouterChange({
  plane,
  by,
  written,
  unreadable = false,
  onClose,
  onChoose,
}: {
  plane: string;
  by: SkillPinner;
  /** What the choice writes now, as written: null for none. */
  written: string | null;
  /** What is written is no router's name at all (not a string): nothing is chosen now, so every
   *  router is a change. */
  unreadable?: boolean;
  onClose: () => void;
  /** The router chosen (`built-in`, an extension's id, or null to take the choice out), and
   *  what its result notice says. */
  onChoose: (router: string | null, said: string) => void;
}) {
  const [offers, setOffers] = useState<{ routers?: SkillRouterOffer[]; refused?: string }>({});
  useEffect(() => {
    let live = true;
    void commands
      .skillRouters(plane)
      .then((answer) => {
        if (!live) return;
        if (answer.status === "ok") setOffers({ routers: answer.data });
        else setOffers({ refused: answer.error });
      })
      .catch((err: unknown) => live && setOffers({ refused: String(err) }));
    return () => {
      live = false;
    };
  }, [plane]);
  const persona = by.at === "persona";
  const now = unreadable ? UNREADABLE : (written ?? (persona ? THE_WORKSPACES : BUILT_IN_ROUTER));
  const [picked, setPicked] = useState(now);
  const routers = offers.routers ?? [];
  // A router written that is not offered here stays listed, so what is chosen now is seen; while
  // the offers are read it is listed without saying it is not offered.
  const unoffered =
    now !== THE_WORKSPACES &&
    now !== BUILT_IN_ROUTER &&
    now !== UNREADABLE &&
    !routers.some((one) => one.id === now)
      ? now
      : undefined;
  const loading = offers.routers === undefined && offers.refused === undefined;
  const whose = whoseSaid(by);
  const pickedSaid = picked === THE_WORKSPACES ? "their workspace's router" : routerSaid(picked);
  const after = `${capitalised(whose)} started after this are routed by ${pickedSaid}.`;
  return (
    <Dialog
      onClose={onClose}
      title="Change the router"
      subtitle={`For ${whose}`}
      description="The router decides which skills a chat sees when its list is over the budget. It is asked at a chat's start."
      summary={
        picked === UNREADABLE
          ? `What is chosen for ${whose} now names no router, so they are routed by the built-in router.`
          : picked === now
            ? `${capitalised(whose)} are routed by ${pickedSaid} now.`
            : `${after} Chats open now keep theirs.`
      }
      main={{
        children: "Change router",
        isDisabled: picked === now,
        onPress: () => onChoose(picked === THE_WORKSPACES ? null : picked, after),
      }}
      data-testid="router-change"
    >
      <RadioGroup aria-label="Router" value={picked} onChange={setPicked}>
        {persona && (
          <Radio
            value={THE_WORKSPACES}
            description="Each workspace's own choice, built-in where it makes none."
          >
            Its workspace's router
          </Radio>
        )}
        <Radio
          value={BUILT_IN_ROUTER}
          description="purlis's own: it lists every skill within the budget, and the rest in an index skill a chat can read."
        >
          Built-in
        </Radio>
        {routers.map((one) => (
          <Radio
            key={one.id}
            value={one.id}
            description={
              !one.reads_prompts
                ? `The extension ${one.name}.`
                : one.prompts_on
                  ? `The extension ${one.name}. You turned on its reading of prompts, so it reads every prompt of the chats it routes.`
                  : `The extension ${one.name}. It can read prompts too, once you turn that on for it.`
            }
          >
            <code>{one.id}</code>
          </Radio>
        ))}
        {unoffered !== undefined && (
          <Radio
            value={unoffered}
            description={
              loading
                ? undefined
                : "Not approved on this machine or not on in this project, so these chats fall back on the built-in router."
            }
          >
            <code>{unoffered}</code>
          </Radio>
        )}
      </RadioGroup>
      {offers.refused !== undefined && (
        <p className="honest">{`purlis could not read the router extensions: ${offers.refused}`}</p>
      )}
      {offers.routers !== undefined && routers.length === 0 && (
        <p className="honest" data-testid="router-change-none">
          No router extension is approved on this machine and on in this project. One that declares
          skill-router is offered here once it is.
        </p>
      )}
    </Dialog>
  );
}

/**
 * **How the router did at each chat's start** (#3692): each chat open here that an extension's
 * router was asked about, with its timing or why it fell back. The built-in router asks nothing
 * and writes no record, so its chats are not listed.
 */
export function RoutedChats({
  chats,
  chosen,
  onChoose,
}: {
  chats: readonly StudioRouted[];
  chosen: StudioRouted | undefined;
  onChoose: (id: string) => void;
}) {
  if (chats.length === 0)
    return (
      <EmptyState
        headline="No chat here was routed by an extension"
        body="The built-in router asks nothing and writes no record. Each chat an extension's router is asked about at its start is listed here, with its timing and what it hid."
        testid="skill-routing-no-chats"
      />
    );
  return (
    <List
      aria-label="Each chat's start"
      className="skill-library"
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={chosen === undefined ? new Set() : new Set([routedChatId(chosen.session)])}
      onSelectionChange={(keys: Selection) => {
        const [first] = keys === "all" ? [] : [...keys];
        if (first !== undefined) onChoose(String(first));
      }}
    >
      {chats.map((chat) => (
        <ListRow
          key={chat.session}
          id={routedChatId(chat.session)}
          textValue={`Chat ${chat.session}`}
          data-testid={`routed-chat-${chat.session}`}
        >
          <RowLabel>{`Chat ${chat.session}`}</RowLabel>
          <RowTrail>{markOf(chat)}</RowTrail>
          <RowBelow clamp={1}>{lineOf(chat)}</RowBelow>
        </ListRow>
      ))}
    </List>
  );
}

/** **A chat's start, beside the list** (#3692): the router asked, how long it took, what came of
 *  it, and every skill it hid. */
export function RoutedChatItem({ chat }: { chat: StudioRouted | undefined }) {
  if (chat === undefined)
    return (
      <EmptyState
        headline="Pick a chat"
        body="How the router did at its start, and what it hid, open here."
        testid="routing-unpicked"
      />
    );
  return (
    <div className="skill-studio-item" data-testid="routed-chat-item">
      <ViewPageHeader
        name={`Chat ${chat.session}`}
        mark={markOf(chat)}
        description={chat.routing.said}
      />
      <div>
        <Facts aria-label={`How chat ${chat.session} was routed`}>
          <Fact label="Router asked">
            <code>{chat.routing.router}</code>
          </Fact>
          <Fact label="Answered in">{`${chat.routing.ms} ms`}</Fact>
          <Fact label="Hid">{chat.hidden.length === 0 ? "Nothing" : chat.hidden.join(", ")}</Fact>
        </Facts>
      </div>
    </div>
  );
}
