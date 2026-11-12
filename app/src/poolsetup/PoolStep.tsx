// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import {
  commands,
  type PlaneId,
  type PoolAdding,
  type PoolSetupShown,
  type PoolSetupStep,
} from "../bindings";
import { Notice } from "../Notice";
import {
  makeRunnerAgain,
  readReleaseImage,
  rereadPools,
  usePools,
  type ReleaseImage,
} from "../pools";
import { PoolLimits } from "../settings/PoolLimits";
import { Button, ButtonRow } from "../ui/button/Button";
import { CheckLine, CheckList } from "../ui/checks/Checks";
import { CodeBlock } from "../ui/code/CodeBlock";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { TextField } from "../ui/field/Field";
import { Action, Options, Recommended, Say, SeeWhat, StepHead } from "./parts";

/** How long typing rests before the name is kept. */
const KEEP_NAME_MS = 400;

const IMAGE_NOT_NAMED = "This build of purlis names no runner image, so give one by its digest.";

/** What the Options hold, as typed. */
export type Chosen = {
  image: "ours" | "team";
  team: string;
  cpus: string;
  memory: string;
  processes: string;
  tmp: string;
  after: string;
  frames: string;
  width: string;
};

export const NOTHING_CHOSEN: Chosen = {
  image: "ours",
  team: "",
  cpus: "",
  memory: "",
  processes: "",
  tmp: "",
  after: "",
  frames: "",
  width: "",
};

/** A pool being made, held above the step so leaving it loses nothing. */
export type Adding = {
  making: boolean;
  refused?: string;
  /** What a team's image breaks, where the last Create stopped on it. */
  breaks?: readonly string[];
  set: (to: { making: boolean; refused?: string; breaks?: readonly string[] }) => void;
  /** The name typed, undefined until something is typed. */
  typed: string | undefined;
  type: (to: string) => void;
  chosen: Chosen;
  choose: (to: Chosen) => void;
};

/** A count typed: `null` for the default, `undefined` where it isn't a whole number. */
function count(typed: string): number | null | undefined {
  const trimmed = typed.trim();
  if (trimmed === "") return null;
  return /^\d+$/.test(trimmed) ? Number(trimmed) : undefined;
}

/** A box of the Options: empty is the recommended value, which it shows and says. */
function Limit({
  label,
  value,
  onChange,
  recommended,
  unit,
  numeric = true,
}: {
  label: string;
  value: string;
  onChange: (to: string) => void;
  recommended: string | number | undefined;
  unit?: string;
  numeric?: boolean;
}) {
  const shown = recommended === undefined ? undefined : String(recommended);
  const wrong = numeric && value.trim() !== "" && count(value) === undefined;
  return (
    <TextField
      label={label}
      value={value}
      onChange={onChange}
      inputMode={numeric ? "numeric" : undefined}
      placeholder={shown}
      help={shown === undefined ? unit : `${unit ? `${unit}. ` : ""}Recommended: ${shown}.`}
      error={wrong ? "Give a whole number, or leave it empty." : undefined}
    />
  );
}

/** Everything a new pool can be given, with the recommended value marked (Q30, story 26). */
function NewPoolOptions({
  chosen,
  choose,
  named,
  version,
  refused,
}: {
  chosen: Chosen;
  choose: (to: Chosen) => void;
  /** purlis's image of this version, where this build names one. */
  named: string | undefined;
  version: string | undefined;
  refused: boolean;
}) {
  const shown = usePools().read;
  const read = shown.state === "read" ? shown.shown : undefined;
  const set = (key: keyof Chosen) => (to: string) => choose({ ...chosen, [key]: to });
  const team = chosen.image === "team" || named === undefined;
  return (
    <Options open={refused || (named === undefined && chosen.team === "")}>
      {named !== undefined && (
        <RadioGroup
          aria-label="Runner image"
          value={chosen.image}
          onChange={(to) => choose({ ...chosen, image: to === "team" ? "team" : "ours" })}
        >
          <Radio
            value="ours"
            description="A ready-made image with git, gh and glab, named by its digest below."
          >
            <span className="pool-setup-choice">
              {version ? `purlis's image, ${version}` : "purlis's image, this version"}
              <Recommended />
            </span>
          </Radio>
          <Radio
            value="team"
            description="An image your team built that keeps the runner image contract."
          >
            My team's own image
          </Radio>
        </RadioGroup>
      )}
      {named !== undefined && !team && (
        // Beside the radio, not in it: a control inside a radio's words is one a reader can't
        // reach (#6206).
        <CodeBlock label="purlis's image, by its digest" text={named} copy="Copy digest" />
      )}
      {team && (
        <TextField
          label="Image, by digest"
          value={chosen.team}
          onChange={set("team")}
          help="purlis checks it keeps the contract before it makes anything."
        />
      )}
      <h2 className="pool-setup-h2">Limits of its runner</h2>
      <p className="pool-setup-does">
        Each is set to what purlis recommends. The limits on what's sent to a runner are under
        Options once the pool exists.
      </p>
      <div className="pool-setup-grid">
        <Limit
          label="CPUs"
          value={chosen.cpus}
          onChange={set("cpus")}
          recommended={read?.defaults.cpus}
          numeric={false}
        />
        <Limit
          label="Memory"
          value={chosen.memory}
          onChange={set("memory")}
          recommended={read?.defaults.memory_mib}
          unit="MiB"
        />
        <Limit
          label="Processes"
          value={chosen.processes}
          onChange={set("processes")}
          recommended={read?.defaults.processes}
        />
        <Limit
          label="/tmp"
          value={chosen.tmp}
          onChange={set("tmp")}
          recommended={read?.defaults.tmp_mib}
          unit="MiB"
        />
        <Limit
          label="Suspends after no link for"
          value={chosen.after}
          onChange={set("after")}
          recommended={read?.suspend_after}
          numeric={false}
        />
        <Limit
          label="Browser frames a second"
          value={chosen.frames}
          onChange={set("frames")}
          recommended={read?.frames ?? undefined}
        />
        <Limit
          label="Browser width"
          value={chosen.width}
          onChange={set("width")}
          recommended={read?.width ?? undefined}
          unit="Pixels"
        />
      </div>
    </Options>
  );
}

/** What `chosen` asks for, or `undefined` while a box holds what no pool could have. */
function asked(name: string, image: string, chosen: Chosen): PoolAdding | undefined {
  const [memory, processes, tmp] = [
    count(chosen.memory),
    count(chosen.processes),
    count(chosen.tmp),
  ];
  const [frames, width] = [count(chosen.frames), count(chosen.width)];
  if ([memory, processes, tmp, frames, width].includes(undefined)) return undefined;
  const text = (typed: string) => (typed.trim() === "" ? null : typed.trim());
  return {
    name: name.trim(),
    image,
    cpus: text(chosen.cpus),
    memory_mib: memory ?? null,
    processes: processes ?? null,
    tmp_mib: tmp ?? null,
    suspend_after: text(chosen.after),
    frames: frames ?? null,
    width: width ?? null,
  };
}

/** The pool that stands: what it is, a change to it, and on to the next step. */
function MadePool({
  step,
  pool,
  noticed,
  plane,
  adding,
  changed,
  onNext,
}: {
  step: PoolSetupStep;
  pool: string;
  noticed: boolean;
  plane: PlaneId;
  adding: Adding;
  changed: () => void;
  onNext: () => void;
}) {
  const action = step.next;
  return (
    <>
      <StepHead
        title={noticed ? "Your pool is ready" : `Pool ${pool}`}
        lead={noticed ? "You ran this one yourself, and purlis noticed." : undefined}
      />
      {step.state === "attention" ? (
        step.findings.map((finding) => (
          <p key={finding} className="pool-setup-p">
            {finding}
          </p>
        ))
      ) : noticed ? (
        <Say tone="news" title={`Found pool ${pool}`}>
          Your setup follows what you did.
        </Say>
      ) : (
        <p className="pool-setup-p">
          Pool {pool} is made, and its runner is on this machine's Docker.
        </p>
      )}
      {adding.refused && (
        <Notice
          cause="pool-setup.pool"
          tone="trouble"
          title="That didn't work"
          onDismiss={() => adding.set({ making: false })}
        >
          {adding.refused}
        </Notice>
      )}
      <Options>
        <PoolLimits pool={pool} plane={plane} />
      </Options>
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      {action?.id === "make-runner-again" ? (
        <Action
          does={action.does}
          main={action.label}
          busy={adding.making}
          onMain={() => {
            adding.set({ making: true });
            void makeRunnerAgain(pool).then((why) => {
              adding.set({ making: false, refused: why });
              changed();
            });
          }}
        />
      ) : action?.id === "check-again" ? (
        <Action does={action.does} main={action.label} onMain={changed} />
      ) : action ? (
        <Action does={action.does} />
      ) : (
        <Action
          main={noticed ? "Continue to sign-ins" : "Continue"}
          onMain={() => {
            if (!noticed) return onNext();
            void commands.poolSetupPool(pool).then(changed);
          }}
        />
      )}
    </>
  );
}

/** The Pool step: a name and its own button, with image and limits under Options (#4481). */
export function PoolStep({
  step,
  shown,
  plane,
  adding,
  changed,
  onNext,
}: {
  step: PoolSetupStep;
  shown: PoolSetupShown;
  plane: PlaneId;
  adding: Adding;
  changed: () => void;
  onNext: () => void;
}) {
  const name = adding.typed ?? shown.name ?? "";
  const keeping = useRef<ReturnType<typeof setTimeout>>(undefined);
  // Kept as it is typed, so a tab closed mid-word comes back with it. A step left before the
  // wait ends keeps the name then, so no wait outlives it (#4823).
  const unkept = useRef<string>(undefined);
  const keep = useCallback(() => {
    clearTimeout(keeping.current);
    const to = unkept.current;
    if (to === undefined) return;
    unkept.current = undefined;
    const kept = to.trim();
    void commands.poolSetupName(kept === "" ? null : kept);
  }, []);
  useEffect(() => keep, [keep]);
  const [offered, setOffered] = useState<ReleaseImage>();
  const [command, setCommand] = useState<string>();
  useEffect(() => {
    if (shown.pool !== null) return;
    let live = true;
    void readReleaseImage().then((read) => {
      if (live) setOffered(read);
    });
    return () => {
      live = false;
    };
  }, [shown.pool]);

  const named = offered && "image" in offered ? offered.image : undefined;
  const reading = offered === undefined;
  const newer = offered?.state === "newer" ? offered : undefined;
  const trouble = offered?.state === "trouble" ? offered.trouble : undefined;
  const { chosen } = adding;
  const team = chosen.image === "team" || named === undefined;
  const using = team ? chosen.team.trim() : (named ?? "");
  // An empty box is the recommended name, one no pool here has (#6256), as an empty limit is its own.
  const recommended = shown.recommended ?? "";
  const naming = name.trim() === "" ? recommended : name;
  const wanted = asked(naming, using, chosen);
  const wantedKey = JSON.stringify(wanted);

  // What See what this does shows is the core's spelling of what Create would send (Q30).
  useEffect(() => {
    if (shown.pool !== null) return;
    const sent: PoolAdding | undefined = JSON.parse(wantedKey ?? "null") ?? undefined;
    let live = true;
    // A box that holds what no pool could have shows no command: never one for what was before.
    if (sent === undefined) {
      queueMicrotask(() => {
        if (live) setCommand(undefined);
      });
      return () => {
        live = false;
      };
    }
    void commands.poolSetupCommand(sent).then((said) => {
      if (live) setCommand(said.status === "ok" ? said.data : undefined);
    });
    return () => {
      live = false;
    };
  }, [shown.pool, wantedKey]);

  if (shown.pool !== null)
    return (
      <MadePool
        step={step}
        pool={shown.pool}
        noticed={shown.noticed}
        plane={plane}
        adding={adding}
        changed={changed}
        onNext={onNext}
      />
    );

  // A name no pool could have yet is left unkept, and Create says why.
  const typing = (to: string) => {
    adding.type(to);
    unkept.current = to;
    clearTimeout(keeping.current);
    keeping.current = setTimeout(keep, KEEP_NAME_MS);
  };
  const taken = shown.others.includes(naming.trim());

  const create = async () => {
    if (naming.trim() === "")
      return adding.set({ making: false, refused: "Give the pool a name first." });
    if (using === "") return adding.set({ making: false, refused: IMAGE_NOT_NAMED });
    if (wanted === undefined)
      return adding.set({ making: false, refused: "A limit under Options isn't a whole number." });
    adding.set({ making: true });
    // Docker may have changed since this step was read: what is made is held to now.
    const now = await commands.poolSetup(plane);
    if (now.status === "error") return adding.set({ making: false, refused: now.error });
    const where = now.data.steps.find((one) => one.id === "where");
    if (where?.state !== "done") {
      adding.set({ making: false });
      changed();
      return;
    }
    const answer = await commands.poolSetupCreate(wanted);
    if (answer.status === "error") return adding.set({ making: false, refused: answer.error });
    if (answer.data.breaks.length > 0)
      return adding.set({ making: false, breaks: answer.data.breaks });
    const going = await commands.poolSetupPool(wanted.name);
    adding.set(
      going.status === "error" ? { making: false, refused: going.error } : { making: false },
    );
    // The rest of the window lists the new pool too.
    await rereadPools();
    changed();
  };

  const breaks = adding.breaks ?? [];
  return (
    <NewPool
      step={step}
      adding={adding}
      breaks={breaks}
      named={named}
      name={name}
      recommended={recommended}
      typing={typing}
      taken={taken}
      others={shown.others}
      changed={changed}
      chosen={chosen}
      team={team}
      newer={newer?.version}
      trouble={trouble}
      reading={reading}
      command={command}
      create={() => void create()}
    />
  );
}

/** What a create said went wrong, moved to as it is said: Create is at the page's foot. */
function useShownWhenSaid(said: unknown) {
  const ref = useRef<HTMLDivElement>(null);
  // What was said before the step was drawn again isn't said again: only a change is.
  const before = useRef(said);
  useEffect(() => {
    if (said && said !== before.current) ref.current?.focus();
    before.current = said;
  }, [said]);
  return ref;
}

function NewPool({
  step,
  adding,
  breaks,
  named,
  name,
  recommended,
  typing,
  taken,
  others,
  changed,
  chosen,
  team,
  newer,
  trouble,
  reading,
  command,
  create,
}: {
  step: PoolSetupStep;
  adding: Adding;
  breaks: readonly string[];
  named: string | undefined;
  name: string;
  /** The name an empty box gives the pool. */
  recommended: string;
  typing: (to: string) => void;
  taken: boolean;
  others: readonly string[];
  changed: () => void;
  chosen: Chosen;
  team: boolean;
  newer: string | undefined;
  trouble: string | undefined;
  reading: boolean;
  command: string | undefined;
  create: () => void;
}) {
  const said = useShownWhenSaid(breaks.length > 0 ? breaks.join("\n") : adding.refused);
  return (
    <>
      <StepHead
        title="Name your pool"
        lead={
          breaks.length > 0
            ? "Your team's image can't be used yet. Nothing was created."
            : "Pick any short name you'll recognise. The rest is already set to what we recommend."
        }
      />
      {adding.making ? (
        <p className="pool-setup-busy" role="status">
          <Loader2 aria-hidden size={18} className="spinning" />
          <span>Making the pool and its runner. The first time, this downloads the runner.</span>
        </p>
      ) : (
        <>
          {breaks.length > 0 && (
            <div ref={said} tabIndex={-1} className="pool-setup-said">
              <Say
                tone="trouble"
                title={`This image is missing ${breaks.length === 1 ? "1 thing" : `${breaks.length} things`} a runner needs`}
                actions={
                  named !== undefined && (
                    <Button
                      onPress={() => {
                        adding.choose({ ...chosen, image: "ours" });
                        adding.set({ making: false });
                      }}
                    >
                      Use purlis's image instead
                    </Button>
                  )
                }
              >
                Ask whoever builds it to fix these, or use purlis's image for now.
              </Say>
              <CheckList aria-label="What the image is missing">
                {breaks.map((problem) => (
                  <CheckLine key={problem} verdict="missing" title={problem} />
                ))}
              </CheckList>
            </div>
          )}
          <TextField
            label="Name"
            value={name}
            onChange={typing}
            placeholder={recommended || undefined}
            help={
              recommended
                ? `Small letters, digits and dashes. Recommended: ${recommended}.`
                : "Small letters, digits and dashes."
            }
            error={
              taken ? `You already have a pool called ${name.trim() || recommended}.` : undefined
            }
          />
          {others.length > 0 && (
            <section className="pool-setup-others" aria-label="Your pools">
              <p className="pool-setup-does">Or carry on with a pool you already have:</p>
              <ButtonRow size="md">
                {others.map((other) => (
                  <Button
                    key={other}
                    onPress={() => void commands.poolSetupPool(other).then(changed)}
                  >
                    {`Use pool ${other}`}
                  </Button>
                ))}
              </ButtonRow>
            </section>
          )}
          <dl className="pool-setup-facts" aria-label="What purlis chose for you">
            <div>
              <dt>Image</dt>
              <dd>{team ? "Your team's own" : "purlis's image, this version"}</dd>
            </div>
            <div>
              <dt>Limits</dt>
              <dd>Recommended, for one person's chats</dd>
            </div>
          </dl>
          {newer && (
            <p className="pool-setup-does">
              {`The image is from purlis ${newer}. An update to it is waiting.`}
            </p>
          )}
          {!reading && named === undefined && (
            <p className="pool-setup-does">{trouble ?? IMAGE_NOT_NAMED}</p>
          )}
          {adding.refused && (
            <div ref={said} tabIndex={-1} className="pool-setup-said">
              <Notice
                cause="pool-setup.pool"
                tone="trouble"
                title="The pool wasn't made"
                onDismiss={() => adding.set({ making: false })}
              >
                {adding.refused}
              </Notice>
            </div>
          )}
          {!reading && (
            <NewPoolOptions
              chosen={chosen}
              choose={adding.choose}
              named={named}
              version={newer}
              refused={breaks.length > 0}
            />
          )}
          {command && <SeeWhat text={command} />}
        </>
      )}
      <Action
        does={step.next?.does}
        main={step.next?.label ?? "Create the pool"}
        busy={adding.making || reading}
        onMain={create}
      />
    </>
  );
}
