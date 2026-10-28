// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The neutral harness model (ADR 0073, FD-13): what a chat's harness tells charter, in
//! charter's words and not any one harness's.
//!
//! Every level speaks it. At level 2 a harness's own hooks report in charter's hook words
//! (`charter hook stop`, the words [`crate::state::Event`] parses), and
//! [`crate::state::Event::said`] reads one of those into this model. At level 3 a protocol client (HP-2 for ACP, HP-3 for a
//! harness's own protocol) will produce the same values, so nothing that reads a chat's state
//! has to know which level or which harness it came from.
//!
//! Six things, as the ticket names them: a **session**, a **turn**, an **item** inside a turn,
//! an **ask** that hands control to the operator, a **plan**, and **usage**. Level 2 carries
//! the first four today. A plan and usage have no hook that reports them as a state charter
//! draws; they are here so that a level-3 adapter has a place to put them, and the chat board
//! moves on none of them.

/// One thing a harness said about a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    Session(Session),
    Turn(Turn),
    Item(Item),
    Ask(Ask),
    Plan(Plan),
    Usage(Usage),
}

/// What happened to the harness's session: the conversation charter follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    /// A session is here: started, resumed, or a new conversation after a clear.
    Began(Began),
    /// The conversation was compacted. Nothing about the turn changed.
    Compacted,
    /// The conversation was cleared away, and another begins at once in the same process.
    /// The session did not end.
    ClearedAway,
    /// The session itself is over.
    Ended,
}

/// How a session came to be here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Began {
    /// Started or resumed, or nothing said which: what a session beginning means when the
    /// harness says nothing else.
    Fresh,
    /// A new conversation in the same process, after the old one was cleared.
    Cleared,
}

/// A turn: one prompt and everything the agent does about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// A prompt was submitted, and the agent is working on it.
    Began,
    /// The agent has nothing more to do, so the next move is the operator's.
    Ended,
    /// The agent stopped for now while helpers it started in the background are still at
    /// work, and the harness wakes it with what they say (#1626). Its work goes on: the next
    /// move is nobody's yet, and the turn that follows their end is the one that ends.
    AwaitsItsHelpers,
    /// The harness nudged that the chat sits idle at its prompt, its turn over (#1626): a
    /// nudge it says is one, never a permission or a question.
    SitsIdle,
}

/// One thing done inside a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A child agent the turn dispatched has finished. The turn that dispatched it goes on.
    ChildEnded,
    /// The harness told the person something that asks nothing of them (#1691): a sign-in
    /// done, a form's answer sent, a wait for a usage limit over. No state moves on it.
    Told,
    /// The harness said the prompt it showed was answered (#1691): a form's answer sent, or
    /// a link's work done. The turn goes on.
    Answered,
}

/// The harness handed control to the operator: a permission, a question, or a nudge that the
/// chat is idle. HP-5 normalises every source into this one shape (`super::asked` reads each
/// harness's own), so the inbox, the audit and every client read an ask the same way whatever
/// harness and level it came from.
///
/// An ask is a value. The id charter answers it by is given when it is raised
/// ([`super::asks::Asks::raise`]), because the sources' own request ids are neither unique
/// across chats nor present at every level. A level-2 `Notification` carries nothing more than
/// that it asked, so it is [`Ask::default`]: no action, no options, answered in the pane.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Ask {
    /// What the agent wants to do.
    pub action: Action,
    /// The answers the source offers, in its order and its words. Empty where the source did
    /// not say, and then the ask is answered in the pane.
    pub options: Vec<Choice>,
    /// Who may answer it.
    pub may_answer: MayAnswer,
    /// How long charter holds it before the source's own prompt decides.
    pub deadline: Deadline,
    /// How dangerous it is to say yes.
    pub risk: Risk,
    /// One line saying what is asked, with every credential shape masked. Kept on this machine.
    pub summary: Summary,
    /// Whether answering it hands over a value a secret could be: such an ask is answered only
    /// by the operator, through a dialog the app owns, so the value reaches the tool and never
    /// the transcript (ADR 0080 §2).
    pub elicits_secret: bool,
    /// Where the answer goes back: the source's own channel, never keystrokes (04 §5.1).
    pub channel: Channel,
    /// What the harness's own terminal shows while it waits, where a level-2 nudge said
    /// (#1691). Unsaid for every other source, whose ask says what it is itself.
    #[serde(default, skip_serializing_if = "Prompt::is_unsaid")]
    pub prompt: Prompt,
    /// What an edit would write, where its source hands it whole (#1146): shown before the
    /// edit may be allowed from the window, or why it cannot be. None for any other ask, and
    /// for an edit whose source names only its path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writes: Option<Writes>,
}

/// **What an edit would write, as the window shows it before it may be allowed** (#1146).
///
/// It is the tool's own input, never the file read from disk: what is shown is what the tool
/// writes, whatever the file holds by the time it runs, and nothing the chat names is read to
/// show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "shown")]
pub enum Writes {
    /// Every change the tool makes, whole, in the order it makes them.
    Whole { changes: Vec<Written> },
    /// Not shown, and why: the edit is answered in its pane.
    NotShown { why: Unshown },
}

/// One change an edit makes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Written {
    /// The whole file becomes `content`, whatever it held before.
    File { content: String },
    /// `before` becomes `after`: where it appears once, or `every` place it appears.
    Replace {
        before: String,
        after: String,
        every: bool,
    },
}

/// Why an edit's change is not shown, and so not allowed from the window (#1146).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unshown {
    /// More lines than the window draws in an ask.
    TooBig,
    /// It holds a NUL, so it is not text.
    Binary,
    /// It holds a character the window draws as nothing or elsewhere: a control other than a
    /// tab, a newline or the carriage return of a line that ends in CRLF, or a format or line
    /// separator character.
    DrawnOtherwise,
    /// It holds what looks like a credential, which the window does not draw.
    Credential,
    /// Its input is not one this reads whole: a field it does not know, one missing or of
    /// another type, or a tool whose change it cannot draw.
    Unread,
}

/// **The prompt a harness shows in its own terminal while it waits on the person** (#1691),
/// as its nudge names it: what the person finds there when they go to the chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Prompt {
    /// The nudge said no kind, or one purlis does not know: still a prompt.
    #[default]
    Unsaid,
    /// A permission for a tool call, or for a sandboxed command's network request.
    Permission,
    /// A form a tool server asks to be filled.
    Form,
    /// A link a tool server asks the person to open.
    Link,
    /// A question, or another input the session waits on.
    Question,
    /// A wait that goes on only once the person presses Enter.
    Continue,
}

impl Prompt {
    /// Whether the nudge named no kind.
    pub fn is_unsaid(&self) -> bool {
        *self == Self::Unsaid
    }

    /// What a row says of a chat stopped on this prompt in its terminal.
    pub fn says(self) -> &'static str {
        match self {
            Self::Unsaid => "Waiting in its terminal",
            Self::Permission => "Waiting in its terminal: a permission prompt",
            Self::Form => "Waiting in its terminal: a form to fill",
            Self::Link => "Waiting in its terminal: a link to open",
            Self::Question => "Waiting in its terminal: a question",
            Self::Continue => "Waiting in its terminal: press Enter to continue",
        }
    }
}

/// What an ask is about.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Action {
    /// The source did not say: a level-2 nudge, or a level-1 harness.
    #[default]
    Unsaid,
    /// Run a command line.
    Command { line: String },
    /// Change a file.
    Edit { path: String },
    /// Call any other tool, with its input as the source gave it (compact JSON).
    Tool { name: String, input: String },
    /// Hand over values the agent asks for: an ACP elicitation's form, by its fields' labels.
    /// The form itself goes to the window with the ask (`crate::acp::Event::Elicited`).
    Elicit { fields: Vec<String> },
    /// Answer a question the agent asks the person, in its words: Claude Code's
    /// `AskUserQuestion` on its permission hook (#1703). The answers are the ask's options.
    Question { question: String },
}

impl Action {
    /// What an ask about this action says, before it is put on one line and masked: the text
    /// its [`Summary`] is made from.
    pub fn said(&self) -> String {
        match self {
            Self::Unsaid => "Asks for you".to_owned(),
            Self::Command { line } => format!("Run a command: {line}"),
            Self::Edit { path } => format!("Change {path}"),
            Self::Tool { name, input } => format!("Use {name}: {input}"),
            Self::Elicit { fields } => format!("Give values for: {}", fields.join(", ")),
            Self::Question { question } => format!("Asks you: {question}"),
        }
    }
}

/// One answer a source offers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    /// What the source is sent back to choose it: its own option id or word.
    pub id: String,
    /// What the source calls it.
    pub label: String,
    pub kind: ChoiceKind,
    /// How long a yes or a no lasts.
    pub scope: ChoiceScope,
}

/// Whether a [`Choice`] lets the action run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceKind {
    Allow,
    Reject,
    /// Stop the turn rather than answer (the Codex app-server's `cancel`).
    Cancel,
}

/// How long a [`Choice`] holds: this once, the rest of the session, or from now on, which a
/// source may keep in the worktree's harness settings (V28c limits that one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceScope {
    Once,
    Session,
    Always,
}

/// Who may answer an ask: the operator, through a human scope, and nobody else for now (V75).
/// Never the agent that asked (V16): [`super::asks::Answerer`] is made only from a connection
/// the host admitted as a human scope. AC-11 may add a persona here, under rules of its own.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MayAnswer {
    /// The operator, from the window or `charter inbox`.
    #[default]
    Operator,
}

/// How long charter holds an ask from when it is raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "until", content = "millis")]
pub enum Deadline {
    /// Until it is answered, withdrawn or stopped. An ACP agent waits for its answer, so its
    /// ask has no deadline and charter adds none (ADR 0080 §5, V28d).
    #[default]
    None,
    /// This many milliseconds, after which the source's own prompt decides.
    Within(u64),
}

/// What charter leaves between an ask's deadline and the hook timeout it sits below: time for
/// the answer to be written back and for the hook to exit before the harness gives up on it.
pub const HOOK_MARGIN: std::time::Duration = std::time::Duration::from_secs(2);

impl Deadline {
    /// The deadline for an ask that came on a hook the harness kills after `timeout`: below it
    /// by [`HOOK_MARGIN`], so charter's answer lands before the harness stops waiting. `None`
    /// when the timeout leaves no room, and then the ask can be answered only in the pane.
    pub fn below_hook_timeout(timeout: std::time::Duration) -> Option<Self> {
        let room = timeout.checked_sub(HOOK_MARGIN)?;
        let millis = u64::try_from(room.as_millis()).ok()?;
        (millis > 0).then_some(Self::Within(millis))
    }

    /// The deadline as a duration, where there is one.
    pub fn duration(self) -> Option<std::time::Duration> {
        match self {
            Self::None => None,
            Self::Within(millis) => Some(std::time::Duration::from_millis(millis)),
        }
    }
}

/// How dangerous saying yes is (N30). A risky ask will need the operator's local verification
/// before its answer counts (SD-18); a normal one needs none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    #[default]
    Normal,
    /// A push to a protected branch.
    ProtectedPush,
    /// Deleting something.
    Delete,
    /// Using or handing over a secret.
    SecretUse,
    /// Acting on a production-like host.
    ProductionHost,
}

impl Risk {
    pub fn is_risky(self) -> bool {
        self != Self::Normal
    }
}

/// One line saying what an ask is, with every credential shape masked, and every vault value
/// this process knows (#1047). It can only be made by masking, read back from the wire
/// included, so no path yields an unmasked one.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(from = "String", into = "String")]
pub struct Summary(String);

/// The most characters a [`Summary`] keeps.
pub const SUMMARY_WIDTH: usize = 200;

/// What a [`Summary`] says where a vault value stood: a mask that names its kind, as a
/// credential shape's does.
pub const VAULT_VALUE: &str = "vault value";

impl Summary {
    /// `text` on one line (its spaces, tabs and line ends collapsed; a blank that only looks like
    /// a space kept, for the window to mark), cut to [`SUMMARY_WIDTH`], with each credential
    /// shape, as written and through its escapes ([`crate::secretshape::leak_spans`], overlapping ones joined),
    /// replaced by a mask that names its kind.
    ///
    /// **And each value this process read from a vault or stored in one**
    /// ([`crate::secrets::seen`], #1047), whatever its shape: a database password or a
    /// passphrase looks like nothing, and the person who revealed it in the window is not shown
    /// it back in what a chat asks to run. A value and a shape that overlap are masked as one,
    /// so no part of either is left beside the mask.
    ///
    /// **It fails closed.** Where the store cannot say what it holds, the whole line is the
    /// mask. So is a line on which a value the text held still stands once it is written: one
    /// that held the value across white space this collapses.
    ///
    /// A masked summary is not its action word for word, so nothing that lets the call run is
    /// offered beside it ([`super::hooked::shown_in_full`]).
    pub fn of(text: &str) -> Self {
        Self(cut(masked(&one_line(text), text)))
    }

    /// **The lines `text` runs over, as its summary shows them** (#3502): where `text` breaks
    /// a line (`\n` or `\r\n`) that the summary puts on one, the count of its lines, and each
    /// one. Blank lines and the blanks that indent a line are collapsed, as on the summary's line.
    ///
    /// **The lines are the summary, split.** They are masked and cut as it is, and each is kept
    /// only where, joined by a space, they are the summary character for character: they never
    /// show a word, or a part of a value, the summary does not. Where they would not be
    /// (masking across a line end reads differently from masking on one line), none is shown,
    /// and only the count is said. Text on one line has none.
    pub fn lines_of(text: &str) -> Option<Lines> {
        let laid = laid_out(text);
        if !laid.contains('\n') {
            return None;
        }
        let lines = masked(&laid, text);
        let shown = cut(lines.clone());
        Some(if shown.replace('\n', " ") == Self::of(text).0 {
            // Counted once masked, so a mask over a line end counts the line it joined once.
            Lines {
                count: lines.split('\n').count(),
                shown: shown.split('\n').map(str::to_owned).collect(),
            }
        } else {
            Lines {
                count: laid.split('\n').count(),
                shown: Vec::new(),
            }
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The lines a text runs over, as [`Summary::lines_of`] shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lines {
    /// How many lines it runs over, counted before the summary is cut: as masked, where the
    /// lines are shown.
    pub count: usize,
    /// Each line as the summary shows it, masked and cut as it is. Empty where they could not
    /// be shown as the summary shows them.
    pub shown: Vec<String>,
}

/// `line`, made from `text`, as a [`Summary`] masks it: each credential shape and each vault
/// value masked, and the whole of it the mask where a value `text` held still stands on it.
fn masked(line: &str, text: &str) -> String {
    let mut spans = crate::secretshape::leak_spans(line);
    spans.extend(
        crate::secrets::seen::found_in(line)
            .into_iter()
            .map(|span| (span, VAULT_VALUE)),
    );
    spans.sort_by_key(|(span, _)| (span.start, std::cmp::Reverse(span.end)));
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    for (span, kind) in spans {
        if span.end <= at {
            continue;
        }
        // One that begins inside the last is joined to it: its mask is already written.
        if span.start >= at {
            out.push_str(&line[at..span.start]);
            out.push_str(&format!("[{kind}]"));
        }
        at = span.end;
    }
    out.push_str(&line[at..]);
    if still_holds_a_vault_value(text, &out) {
        out = format!("[{VAULT_VALUE}]");
    }
    out
}

/// `out` cut to [`SUMMARY_WIDTH`] characters, the last an ellipsis where it was cut.
fn cut(mut out: String) -> String {
    if out.chars().count() > SUMMARY_WIDTH {
        out = out.chars().take(SUMMARY_WIDTH - 1).collect();
        out.push('…');
    }
    out
}

/// `text` with the white space a command line is laid out with collapsed ([`one_line`]): each
/// run of spaces, tabs and line ends (`\n` or `\r\n`) is one line end where it held one, and
/// one space otherwise. The lines a reader is shown ([`Summary::lines_of`]).
fn laid_out(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = None;
    for c in text.replace("\r\n", "\n").chars() {
        match c {
            '\n' => gap = Some('\n'),
            ' ' | '\t' => {
                gap.get_or_insert(' ');
            }
            _ => {
                if let Some(gap) = gap.take()
                    && !out.is_empty()
                {
                    out.push(gap);
                }
                out.push(c);
            }
        }
    }
    out
}

/// `text` on one line: the white space a command line is laid out with collapsed to one space.
/// That is a space, a tab and a line end (`\n` or `\r\n`). Any other blank stays where it
/// stood: one that only looks like a space (a no-break, an en or an ideographic space, SK-51,
/// #3202), and a break a shell does not lay a line out with (a lone carriage return, a vertical
/// tab, a form feed, a next line, a line or paragraph separator, #3470). Purlis's one rule
/// marks it where the line is shown, so the window can say why it is not offered Allow.
fn one_line(text: &str) -> String {
    laid_out(text).replace('\n', " ")
}

/// Whether `written`, the masked line made from `text`, still shows a vault value `text` held:
/// one found in `text` whose words, on one line, stand in `written`. The masks hide every
/// value found on the line itself, so this is a value the text held across white space.
fn still_holds_a_vault_value(text: &str, written: &str) -> bool {
    crate::secrets::seen::found_in(text)
        .into_iter()
        .any(|span| {
            // Put on one line the way the summary was, so a value across a kept blank reads
            // as it does there.
            let held = one_line(&text[span]);
            !held.is_empty() && written.contains(&held)
        })
}

impl From<String> for Summary {
    fn from(text: String) -> Self {
        Self::of(&text)
    }
}

impl From<Summary> for String {
    fn from(summary: Summary) -> Self {
        summary.0
    }
}

/// Where an ask came from and its answer goes back, with the source's own request id where it
/// has one. HP-6 and HP-16 answer through it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "via")]
pub enum Channel {
    /// The chat's pane: a level-1 harness, or a level-2 nudge with nothing structured in it.
    #[default]
    Pane,
    /// The hook that asked, answered on its own connection (Claude Code's or Codex's
    /// `PermissionRequest`).
    Hook,
    /// The Codex app-server's approval request.
    CodexAppServer {
        thread: String,
        turn: String,
        item: String,
    },
    /// opencode's permission request, answered on its server.
    Opencode { session: String, request: String },
    /// ACP's `session/request_permission`, answered on the agent's stdio.
    Acp { session: String, tool_call: String },
}

impl Channel {
    /// The source's own id for the request, where it has one: two asks with the same one are
    /// the same request, so the later supersedes the earlier.
    pub fn request(&self) -> Option<(&str, &str)> {
        let (source, request) = match self {
            Self::Pane | Self::Hook => return None,
            Self::CodexAppServer { thread, item, .. } => (thread, item),
            Self::Opencode { session, request } => (session, request),
            Self::Acp { session, tool_call } => (session, tool_call),
        };
        // A payload that left an id out names no request, and so is never the same as another.
        (!source.is_empty() && !request.is_empty()).then_some((source.as_str(), request.as_str()))
    }
}

/// The steps the agent says it will take, in its order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
}

/// One step of a [`Plan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub text: String,
    pub done: bool,
}

/// What a turn cost, as far as the harness said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// How much of the context window is used, as a whole percentage.
    pub context_percent: Option<u8>,
}
