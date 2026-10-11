import { useRef, useState, type ReactNode } from "react";
import { OctagonAlert, TriangleAlert } from "lucide-react";
import { commands, type AlertRow, type DoctorFixed, type PlaneId } from "./bindings";
import type { AlertsReading } from "./alerts";
import { identityNowOf, sendIdentity, useFixForm, type Fixer } from "./Doctor";
import { drawWhatIsInForce } from "./Extensions";
import { Notice, type NoticeAction } from "./Notice";
import {
  sayAboutThisMachine,
  usingTheBuiltIn,
  usingTheDefaultLayout,
  type MachineAlert,
} from "./windowprefs";

/**
 * **What purlis says is wrong, as Notices in the Inbox** (#1695, D-1695-1): the rows the Alerts
 * drawer listed (NO-6, #1238), folded into the one list that waits on the person (spec #1688,
 * I-4). Each is a Notice of the Inbox's, so it stands with the project's other Notices, the most
 * important first, and the status line's Notices button counts it.
 *
 * - **This project's alerts**: its pin, its front door, its workspaces' layout, its root. The
 *   core decides them (`purlis_core::alerts`, the same rows `purlis statusline` draws), and
 *   each carries the way out the core gave it (`AlertRow.way`): a Settings group or setting, a
 *   fix of the doctor's registry (with its form, #1301), the outer project, the Saving view.
 * - **This machine's**: a theme or layout file purlis could not use as written, with its
 *   Settings group, Use the built-in theme or Use the default layout (each asks first), or
 *   Dismiss.
 * - **Another project's**: the drawer listed every project's, because the one that matters is
 *   often not the one in front. Here each other project with alerts is one Notice, "N alerts in
 *   name", whose way out opens that project's Inbox, where they are listed whole.
 * - **What could not be read**, said as trouble with Read again: an alert purlis could not read
 *   is never a silent nothing.
 *
 * An alert is a standing state, not an event: it goes when it is fixed, and is never noted as
 * an update kept for a day (D-1695-1). Nothing here is answered in bulk.
 */
export function InboxAlerts({
  plane,
  reading,
  machine,
  elsewhere,
  does,
}: {
  plane: PlaneId;
  /** Every open project's alerts, as the window last read them. */
  reading: AlertsReading;
  /** What the window says about this machine (`windowprefs.ts`). */
  machine: readonly MachineAlert[];
  /** The other projects this window holds, with what each is called on its tab. */
  elsewhere: readonly { plane: PlaneId; name: string }[];
  does: InboxAlertsDo;
}) {
  return (
    <>
      {machine.map((alert) => (
        <MachineRow key={alert.subject} alert={alert} onOpenSettings={does.openSettings} />
      ))}
      <ThisProject plane={plane} reading={reading} does={does} />
      {reading.at === "read" &&
        elsewhere.map(({ plane: other, name }) => {
          const read = reading.planes.find((one) => one.plane === other);
          const count = read?.alerts.length ?? 0;
          if (count === 0 && (read?.stopped ?? null) === null) return null;
          return (
            <Notice
              key={other}
              cause={`alerts-elsewhere:${other}`}
              tone={read?.alerts.some((one) => one.severity === "bad") ? "trouble" : "news"}
              link={{ label: `See them in ${name}`, onPress: () => does.openInboxOf(other) }}
            >
              {count === 1 ? `1 alert in ${name}` : `${count} alerts in ${name}`}
              {read?.stopped != null && ", and purlis stopped looking there"}
            </Notice>
          );
        })}
    </>
  );
}

/** **Every open project's alerts, as the window holds them** (#1695): read once for the window
 *  (`alerts.ts`), and handed to each project, whose Inbox lists its own. */
export type WindowAlerts = {
  reading: AlertsReading;
  /** The projects this window holds, in the strip's order. */
  planes: readonly PlaneId[];
  /** What a project is called on its tab. */
  nameOf: (plane: PlaneId) => string;
  does: InboxAlertsDo;
};

/** What the alerts' ways out do in the window (NO-6). */
export type InboxAlertsDo = {
  /**
   * Opens Settings at a group (SE-22): a You group, or — with `plane` — that project's; and,
   * with `setting`, focuses that setting of the group (#1289), by the id the Settings builders
   * give it.
   */
  openSettings: (group: string, plane?: PlaneId, setting?: string) => void;
  /** Opens another project by its path, through the window's one way in (the trust gate). */
  openProject: (path: string) => void;
  /** Opens a project's Saving view. */
  openSaving: (plane: PlaneId) => void;
  /** Reads every project's alerts again: after a fix, so a row it cured goes. */
  reread: () => void;
  /** Brings another open project to the front, with its Inbox open. */
  openInboxOf: (plane: PlaneId) => void;
};

/** This project's alerts, or why there are none to list. */
function ThisProject({
  plane,
  reading,
  does,
}: {
  plane: PlaneId;
  reading: AlertsReading;
  does: InboxAlertsDo;
}) {
  const again: [NoticeAction] = [{ label: "Read again", onPress: does.reread }];
  // Still reading: nothing to say yet, and nothing is counted as none (`StatusLine`).
  if (reading.at === "reading") return null;
  if (reading.at === "failed")
    return (
      <Notice cause="alerts-unread" tone="trouble" fixes={again}>
        purlis could not read the alerts: {reading.why}
      </Notice>
    );
  const read = reading.planes.find((one) => one.plane === plane);
  // Opened after the last reading was taken: the next one, which opening it asked for, has it.
  if (read === undefined) return null;
  return (
    <>
      {read.stopped !== null && (
        <Notice cause="alerts-stopped" tone="trouble" fixes={again}>
          purlis stopped looking for alerts here: {read.stopped}. The alerts listed are the ones it
          found before that, and there may be more.
        </Notice>
      )}
      {read.alerts.map((alert) => (
        <ProjectRow
          key={`${alert.subject}\n${alert.detail}`}
          plane={plane}
          alert={alert}
          does={does}
        />
      ))}
    </>
  );
}

/**
 * The words on a fix's button, by its id in the doctor's registry (FX-1): a verb and what it
 * acts on (`docs/ui-copy.md`, #1719), never "Fix" alone. A fix that opens a form says so with
 * its ellipsis.
 */
const FIX_LABELS: Readonly<Record<string, string>> = {
  "workspace-reinit": "Update workspace layout",
  "git-identity": "Set git identity…",
  "memory-optimize": "Optimize memory indexes",
};

/** A fix this window has no words of its own for yet: still a verb and its object. */
const ANY_FIX = "Apply the doctor's fix";

/**
 * What one row says, inside its Notice: its mark, what it is about and what is wrong, then what
 * the last press answered. Each row draws its own `<Notice>` with its ways out named, never
 * spread, so `Notice.guard.test.ts` reads every one of them.
 */
function Words({
  severity,
  subject,
  detail,
  then,
  said,
}: {
  severity: string;
  subject: string;
  detail: ReactNode;
  then?: string;
  said?: string;
}) {
  return (
    <>
      {severity === "bad" ? (
        <OctagonAlert className="node-icon alert-mark" data-severity="bad" />
      ) : (
        <TriangleAlert className="node-icon alert-mark" data-severity="warn" />
      )}
      <span className="alert-words">
        <span className="alert-subject">{subject}</span>{" "}
        <span className="alert-detail">{detail}</span>
        {then !== undefined && <span className="alert-detail"> — {then}</span>}
        {said !== undefined && <span className="alert-said">{said}</span>}
      </span>
    </>
  );
}

/** A row's Notice look: trouble for what loses work if it is left. */
const toneOf = (severity: string) => (severity === "bad" ? "trouble" : "news");

/** A project's row, with the way out the core gave it. */
function ProjectRow({
  plane,
  alert,
  does,
}: {
  plane: PlaneId;
  alert: AlertRow;
  does: InboxAlertsDo;
}) {
  /** What the last fix answered, when it did not cure the row: its refusal, or what it said. */
  const [said, setSaid] = useState<string>();
  const fixing = useRef(false);
  const [busy, setBusy] = useState(false);
  const way = alert.way;
  const label = way.kind === "fix" ? (FIX_LABELS[way.id] ?? ANY_FIX) : "";
  /** The act, as the sentence says it: its verb in lower case, and no ellipsis. */
  const act = label.charAt(0).toLowerCase() + label.slice(1).replace(/…$/, "");
  const could = (why: string) => `purlis could not ${act}: ${why}`;
  /** What a fix came to, said on the row when it did not cure it; then every project is read
   *  again, because a fix that half-ran changed something too. */
  const landed = (fixed: DoctorFixed) => {
    if (fixed.refused !== null) setSaid(could(fixed.refused));
    else if (!fixed.complete) setSaid(fixed.said.join(" "));
    does.reread();
  };
  // **A fix that takes input opens its form here too** (#1301): the row's fix goes through the
  // doctor's own `useFixForm`, so a fix in `FIXES_WITH_A_FORM` is never applied bare (and
  // refused for the input it lacks). Every row asks for it, as hooks must; only a fix row uses it.
  const fixer: Fixer = {
    apply: (id) => {
      if (fixing.current) return;
      fixing.current = true;
      setBusy(true);
      setSaid(undefined);
      void commands
        .planeDoctorFix(plane, id)
        .then((answer) =>
          landed(
            answer.status === "ok"
              ? answer.data
              : { fix: id, refused: answer.error, said: [], complete: false },
          ),
        )
        .catch((err: unknown) =>
          landed({ fix: id, refused: String(err), said: [], complete: false }),
        )
        .finally(() => {
          fixing.current = false;
          setBusy(false);
        });
    },
    busy,
    identity: async (name, email) => {
      setSaid(undefined);
      setBusy(true);
      const sent = await sendIdentity(plane, name, email);
      setBusy(false);
      if ("refused" in sent) return sent.refused;
      landed(sent.fixed);
      return undefined;
    },
    identityNow: () => identityNowOf(plane),
  };
  const fix = useFixForm(way.kind === "fix" ? way.id : "", fixer);
  let action: NoticeAction;
  switch (way.kind) {
    case "settings":
      action = {
        label: "Fix it in Settings",
        onPress: () => does.openSettings(way.group, plane, way.setting ?? undefined),
      };
      break;
    case "open-project":
      action = { label: "Open the outer project", onPress: () => does.openProject(way.path) };
      break;
    case "saving":
      action = { label: "Go to Saving", onPress: () => does.openSaving(plane) };
      break;
    case "fix":
      action = { label, onPress: fix.press, opens: fix.opens };
      break;
  }
  const cause = `alert:${alert.subject}`;
  const tone = toneOf(alert.severity);
  const words = (
    <Words severity={alert.severity} subject={alert.subject} detail={alert.detail} said={said} />
  );
  return way.kind === "fix" ? (
    <Notice cause={cause} tone={tone} fixes={[action]} under={fix.form}>
      {words}
    </Notice>
  ) : (
    <Notice cause={cause} tone={tone} link={action}>
      {words}
    </Notice>
  );
}

/**
 * A file of this machine's that a row can move aside (NO-6, #1289): the theme file, for Use the
 * built-in theme, and the layout file, for Use the default layout. Each asks first, in the row,
 * with what moving it aside does, one claim to a sentence (#1719).
 */
type Aside = {
  /** The press that asks. */
  ask: string;
  /** The answer that moves it. */
  yes: string;
  /** What the question says. */
  question: string;
  move: () => ReturnType<typeof commands.useBuiltInTheme>;
  /** What the window does once it has moved. */
  moved: () => void;
};

const THEME_ASIDE: Aside = {
  ask: "Use the built-in theme…",
  yes: "Use the built-in theme",
  question:
    "Use the built-in theme? purlis moves the theme file aside to theme.aside.json, or to the next free theme.aside-N.json. It never writes over a file. Then it draws what is in force without the file.",
  move: () => commands.useBuiltInTheme(),
  moved: () => {
    usingTheBuiltIn();
    void drawWhatIsInForce();
  },
};

/**
 * **What moving the layout aside loses is said in the question** (D-1289-1): the file keeps more
 * than the arrangement. The text sizes, your editor, how chats are listed, the Notices dismissed
 * in each project and the ones seen once on this machine go aside with it, so a Notice a
 * dismissal was hiding can show again. The window keeps drawing what it has, and its next change
 * writes a new file from it, so the question says that too.
 */
const LAYOUT_ASIDE: Aside = {
  ask: "Use the default layout…",
  yes: "Use the default layout",
  question:
    "Use the default layout? purlis moves the layout file aside to layout.aside.json, or to the next free layout.aside-N.json. It never writes over a file. The file keeps more than the arrangement: the text sizes, your editor, how chats are listed, the Notices you dismissed and the ones seen once on this machine. All of it goes aside with the file, so a Notice you dismissed can show again. The window keeps what it draws now. The next change you make writes a new file. With no change, the next launch starts from the defaults.",
  move: () => commands.useDefaultLayout(),
  moved: usingTheDefaultLayout,
};

/**
 * A row about this machine: its Settings group, a file it can move aside (which asks first, in
 * the row), or — with neither — Dismiss, which takes it back for this launch. A layout row that
 * can move its file aside keeps its Dismiss too (D-1289-2): a machine row's Dismiss lasts one
 * launch, since the layout file is where dismissals are kept, and a broken one cannot hold its
 * own.
 */
function MachineRow({
  alert,
  onOpenSettings,
}: {
  alert: MachineAlert;
  onOpenSettings: (group: string) => void;
}) {
  const [asking, setAsking] = useState(false);
  const [said, setSaid] = useState<string>();
  const aside = alert.builtIn ? THEME_ASIDE : alert.defaultLayout ? LAYOUT_ASIDE : undefined;
  const moveAside = (aside: Aside) => {
    setAsking(false);
    void aside
      .move()
      .then((answer) => {
        if (answer.status === "error") {
          setSaid(answer.error);
          return;
        }
        aside.moved();
      })
      .catch((err: unknown) => setSaid(String(err)));
  };
  const cause = `alert:${alert.subject}`;
  const tone = toneOf(alert.severity);
  const shown = { severity: alert.severity, subject: alert.subject, said };
  const words = <Words {...shown} detail={alert.detail} then={alert.remedy} />;
  const settings = alert.settings;
  const dismiss = () => sayAboutThisMachine(alert.subject, undefined);
  if (asking && aside !== undefined)
    return (
      <Notice
        cause={cause}
        tone={tone}
        // The way out first, the act last (#1719), as every question in the window.
        fixes={[
          { label: "Keep it", onPress: () => setAsking(false) },
          { label: aside.yes, onPress: () => moveAside(aside) },
        ]}
      >
        <Words {...shown} detail={aside.question} />
      </Notice>
    );
  if (settings !== undefined)
    return (
      <Notice
        cause={cause}
        tone={tone}
        link={{ label: "Fix it in Settings", onPress: () => onOpenSettings(settings) }}
      >
        {words}
      </Notice>
    );
  if (aside !== undefined) {
    const ask: [NoticeAction] = [
      {
        label: aside.ask,
        onPress: () => {
          setSaid(undefined);
          setAsking(true);
        },
      },
    ];
    return aside === LAYOUT_ASIDE ? (
      <Notice cause={cause} tone={tone} fixes={ask} onDismiss={dismiss}>
        {words}
      </Notice>
    ) : (
      <Notice cause={cause} tone={tone} fixes={ask}>
        {words}
      </Notice>
    );
  }
  return (
    <Notice cause={cause} tone={tone} onDismiss={dismiss}>
      {words}
    </Notice>
  );
}
