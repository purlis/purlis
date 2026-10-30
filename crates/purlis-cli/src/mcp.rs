// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter mcp`: charter's MCP server over stdio (HP-7, Q15), the one action channel every
//! harness reaches the same way. Each harness adapter starts it for one chat
//! ([`purlis_core::chattools`] has how, and what each tool may do); nobody types it.
//!
//! **stdout is the protocol.** Nothing here prints: a tool's answer is the text it returns,
//! and a line on stdout that is not JSON-RPC would end the connection.
//!
//! **The chat's place is read at every call**, by the same ladder a `charter` command run in the
//! chat reads ([`crate::Here`]), so a tool reaches exactly what `charter … ` with no `-w`
//! reaches, and a `charter workspace use` in the chat moves both together.
//!
//! **A question waiting on the person is said to the app** (#1089), so the chat shows in needs
//! you while `ask_operator` waits, and leaves it once answered ([`Waiting`]). Claude Code says
//! it itself; for a harness with no notice of its own, this server says it.

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ElicitRequestParams,
    ElicitResult, ElicitationAction, ElicitationSchema, Implementation, InputRequest,
    InputRequests, InputRequiredResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
    ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler, ServiceExt};

use purlis_core::chattools;

/// The key an `ask_operator` answer comes back under, at protocol versions that ask by
/// returning `input_required` (SEP-2322).
const ANSWER: &str = "answer";

/// Serves the tools on stdin and stdout until the harness closes them.
pub fn serve(graded: bool) -> Result<u8, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("could not start the MCP server: {e}"))?;
    let waiting = std::sync::Arc::new(Waiting::of_this_chat());
    let server = Server {
        waiting: std::sync::Arc::clone(&waiting),
        graded,
    };
    let served = runtime.block_on(async {
        let running = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| format!("the MCP client did not start a session: {e}"))?;
        running
            .waiting()
            .await
            .map_err(|e| format!("the MCP session ended badly: {e}"))?;
        Ok(0)
    });
    // A question still open when the harness lets go is waited on by nobody.
    waiting.over();
    served
}

/// The server. It holds nothing of the chat's place: every call reads it afresh. It holds only
/// whether a question of its waits on the person ([`Waiting`]).
struct Server {
    waiting: std::sync::Arc<Waiting>,
    /// A grade's chat (SK-16): only the tools that only read are listed and answered
    /// ([`chattools::PRE_ALLOWED`]); none writes, dispatches, asks the person or drives a
    /// browser.
    graded: bool,
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                chattools::SERVER,
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(instructions())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // The skill tools (SK-13), only where the project's skills reach this chat: the
        // `skill-studio` flag, or a project that requires them (#2784).
        let skills: &[chattools::Tool] = if skills_reach_here() {
            &purlis_core::skilltools::TOOLS
        } else {
            &[]
        };
        let graded = self.graded;
        Ok(ListToolsResult::with_all_items(
            chattools::TOOLS
                .iter()
                .chain(purlis_core::browsertools::TOOLS.iter())
                .chain(purlis_core::themepreview::TOOLS.iter())
                .chain(skills.iter())
                .filter(|one| !graded || chattools::PRE_ALLOWED.contains(&one.name))
                .map(tool)
                .collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Any call after a question asked by returning `input_required` is the harness back from
        // showing it: the retry that carries the answer, or a turn that went on without one.
        self.waiting.heard_again();
        let name = request.name.as_ref();
        // A grade's chat is answered the tools that only read, and nothing else (SK-16).
        if self.graded && !chattools::PRE_ALLOWED.contains(&name) {
            return Err(ErrorData::invalid_params(
                format!("purlis has no tool {name:?} for a grade's chat"),
                None,
            ));
        }
        // A browser tool (#2013): handed to the app, which decides and answers it, a
        // screenshot attached as an image.
        if purlis_core::browsertools::is_one(name) {
            let name = name.to_owned();
            let args = request.arguments.clone().unwrap_or_default();
            let (said, attached) =
                tokio::task::spawn_blocking(move || crate::browsing::tool(&name, &args))
                    .await
                    .unwrap_or_else(|e| {
                        (
                            Err(format!("the tool failed inside purlis: {e}")),
                            Vec::new(),
                        )
                    });
            return Ok(done_with(said, attached));
        }
        // The theme specimen (#2330): handed to the app, which draws it in a hidden window of
        // its own, each picture attached as an image.
        if purlis_core::themepreview::is_one(name) {
            let args = request.arguments.clone().unwrap_or_default();
            let (said, attached) =
                tokio::task::spawn_blocking(move || crate::themepreview::tool(&args))
                    .await
                    .unwrap_or_else(|e| {
                        (
                            Err(format!("the tool failed inside purlis: {e}")),
                            Vec::new(),
                        )
                    });
            return Ok(done_with(said, attached));
        }
        // The chat's own skills (SK-13): read, found or loaded, over its own skill set.
        if purlis_core::skilltools::is_one(name) && skills_reach_here() {
            let name = name.to_owned();
            let args = request.arguments.clone().unwrap_or_default();
            return Ok(done(blocking(move || skill_tool(&name, &args)).await));
        }
        if !chattools::TOOLS.iter().any(|t| t.name == name) {
            return Err(ErrorData::invalid_params(
                format!("purlis has no tool {name:?}"),
                None,
            ));
        }
        let args = request.arguments.clone().unwrap_or_default();
        // Pointed at one of the chat's browsers (#2015): the app asks the person to take it
        // over, under Needs you, and answers at once; the harness's prompt asks nothing.
        if name == chattools::ASK_OPERATOR && purlis_core::browsertools::points_at_a_browser(&args)
        {
            return Ok(done(
                blocking(move || crate::browsing::needs_you(&args)).await,
            ));
        }
        if name == chattools::ASK_OPERATOR {
            return ask_operator(&request, &args, &context, &self.waiting).await;
        }
        // FR-24 (#1051): on a project this purlis cannot write, every tool that writes is
        // refused here, before it is run, whatever road it would write by; the read tools
        // answer. Read at every call: the project can be moved to a newer format mid-chat.
        if chattools::TOOLS
            .iter()
            .any(|t| t.name == name && !t.read_only)
            && let Some(why) = blocking(read_only_here).await.err()
        {
            return Ok(done(Err(format!("nothing was written: {why}"))));
        }
        if name == chattools::SESSION_RECORD {
            return Ok(done(blocking(move || session_record(&args)).await));
        }
        if name == chattools::PERSONA_REMEMBER {
            return Ok(done(blocking(move || persona_remember(&args)).await));
        }
        if name == chattools::WORKSPACE_VISION || name == chattools::WORKSPACE_SECTION {
            let name = name.to_owned();
            return Ok(done(blocking(move || workspace_write(&name, &args)).await));
        }
        if name == chattools::PERSONA_WHERE {
            let (said, attached) = tokio::task::spawn_blocking(crate::whereworking::tool)
                .await
                .unwrap_or_else(|e| {
                    (
                        Err(format!("the tool failed inside purlis: {e}")),
                        Vec::new(),
                    )
                });
            return Ok(done_with(said, attached));
        }
        if name == chattools::DISPATCH {
            return Ok(done(blocking(move || dispatch(&args)).await));
        }
        if name == chattools::WORKTREE_UPDATE {
            return Ok(done(
                blocking(move || {
                    let step = chattools::worktree_update_args(&args)?;
                    crate::piece::update_by_the_app(&crate::Here::read()?, &step)
                })
                .await,
            ));
        }
        if name == chattools::WORKTREE_REMOVE {
            return Ok(done(
                blocking(move || {
                    let (repo, piece, delete_branch) = chattools::worktree_remove_args(&args)?;
                    crate::piece::remove_by_the_app(
                        &crate::Here::read()?,
                        &repo,
                        &piece,
                        delete_branch,
                    )
                })
                .await,
            ));
        }
        if name == chattools::DISPATCH_LIST {
            return Ok(done(
                blocking(|| crate::dispatch::list().map_err(|why| in_the_chat(&why))).await,
            ));
        }
        if name == chattools::DISPATCH_READ {
            return Ok(done(blocking(move || dispatch_read(&args)).await));
        }
        if name == chattools::DISPATCH_REPORT {
            return Ok(done(blocking(move || dispatch_report(&args)).await));
        }
        // Every other `purlis dispatch` subcommand, as a tool (#3786): the command's own
        // function, so the tool and the command cannot answer one ask two ways.
        if let Some(run) = task_tool(name) {
            return Ok(done(
                blocking(move || run(&args).map_err(|why| in_the_chat(&why))).await,
            ));
        }
        let name = name.to_owned();
        Ok(done(
            blocking(move || {
                let here = crate::Here::read()?;
                let place = here.place(None);
                chattools::call(
                    here.plane.root(),
                    &place,
                    &name,
                    &args,
                    chrono::Local::now().naive_local(),
                )
            })
            .await,
        ))
    }
}

/// Whether the project's skills reach this chat ([`purlis_core::skills::reach_chats_here`]): the
/// skill tools are listed and answered only then. Outside a plane, as the flag says.
fn skills_reach_here() -> bool {
    match crate::Here::read() {
        Ok(here) => purlis_core::skills::reach_chats_here(here.plane.root()),
        Err(_) => purlis_core::skills::studio_on_here(),
    }
}

/// What the server tells the harness it is for: its tools, and for a chat on the skills'
/// `Mcp` route the one line that says how its skills are reached
/// ([`purlis_core::skilltools::briefing`]): a harness that runs none of purlis's hooks is briefed
/// by nothing else.
fn instructions() -> String {
    let mut said = "purlis's tools for this chat: the todos, memory, session records and changes \
                    of the workspace it works in, a question for the operator, a real browser \
                    the person watches (the browser_ tools), and the theme specimen \
                    (theme_preview)."
        .to_owned();
    if let Ok(here) = crate::Here::read()
        && purlis_core::skills::reach_chats_here(here.plane.root())
        && let Some(line) = purlis_core::skilltools::briefing(route_here(&here))
    {
        said.push_str("\n\n");
        said.push_str(line);
    }
    said
}

/// The chat's harness, by the word the app named it with.
fn this_harness() -> Option<String> {
    purlis_core::hookwire::chat_harness(&purlis_core::envvar::var)
}

/// The route this chat's skills take, by its harness and the project's harness declarations.
fn route_here(here: &crate::Here) -> Option<purlis_core::skills::Route> {
    purlis_core::skills::route_of(
        this_harness().as_deref(),
        &purlis_core::harness_declaration::read(here.plane.root()),
    )
}

/// One of the skill tools, for the chat where it works now; a load's run is opened as a hook
/// opens one ([`purlis_core::skilltools::start_run`]).
fn skill_tool(
    name: &str,
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    let here = crate::Here::read()?;
    let workspace = here.workspace_if_any(None);
    let persona = here.active_persona(None);
    let harness = this_harness();
    let built_in = purlis_core::skills::built_in_here();
    let chat = purlis_core::skilltools::Asking {
        plane: here.plane.root(),
        workspace: workspace.as_deref(),
        persona: persona.as_deref(),
        harness: harness.as_deref(),
        route: route_here(&here),
        built_in: built_in.as_deref(),
    };
    purlis_core::skilltools::call(&chat, name, args, &|skill| {
        crate::hooks::with_hook("", None, |hook| {
            purlis_core::skilltools::start_run(hook, skill);
        });
    })
}

/// `Err` with why, where the project this chat works in is one this purlis cannot write
/// (FR-24); `Ok` where it can, or where no project is found (each tool then says so itself).
fn read_only_here() -> Result<String, String> {
    let Ok(here) = crate::Here::read() else {
        return Ok(String::new());
    };
    match purlis_core::compat::read(here.plane.root()) {
        purlis_core::compat::Compat::ReadOnly(why) => Err(why.to_string()),
        purlis_core::compat::Compat::Writable => Ok(String::new()),
    }
}

/// `body`, off the runtime's one thread, so its file I/O never stalls a question in flight,
/// and with a panic answered as a tool error rather than a call that never returns. (A release
/// build aborts on a panic, so there the server ends and the harness says so; it still never
/// hangs.)
async fn blocking(
    body: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> Result<String, String> {
    tokio::task::spawn_blocking(body)
        .await
        .unwrap_or_else(|e| Err(format!("the tool failed inside purlis: {e}")))
}

/// `session_record`: the one operation `purlis session record` run in the chat performs
/// ([`crate::session::write`]), so the app that started the chat writes it over the chat's hook
/// socket where it can, and this server writes it where it cannot (#1332). Answered with what
/// the command prints: the record, then what becomes of the tab.
fn session_record(args: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let (title, body, pieces) = chattools::record_args(args)?;
    let here = crate::Here::read()?;
    // Outside the sandbox: in a chat the app started, never written here (#1408).
    purlis_core::sessionrecord::check(&title, &body)
        .map_err(|why| format!("nothing was written: {why}"))?;
    match crate::session::write(&here, &title, &body, &pieces, None, None, true) {
        Ok(saved) => {
            let mut said = vec![format!("Session record → {}", saved.shown)];
            said.extend(saved.warnings);
            said.push(saved.tab);
            Ok(said.join("\n"))
        }
        Err(crate::session::NotWritten::Refused(why) | crate::session::NotWritten::Failed(why)) => {
            Err(format!("nothing was written: {why}"))
        }
    }
}

/// `persona_remember`: the one operation `purlis persona remember` run in the chat performs, so
/// the app that started the chat writes it (a brokered write, #1333). Answered with what the
/// command says.
///
/// **This server runs outside the chat's sandbox, so in a chat the app started it never writes
/// itself.** It writes only where there is no app that could have: no chat, or an app that has
/// gone. A server handed no connection to the app, an app that dropped the ask, or one that did
/// not answer, is refused in a sentence: a write made here would be one no
/// sandbox bounds, no rate holds and the trace does not credit.
fn persona_remember(args: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    use crate::brokered::Forwarded;
    let (text, title, shared) = chattools::persona_remember_args(args)?;
    let write = purlis_core::brokered::Write::PersonaRemember {
        text: text.clone(),
        title: title.clone(),
        shared,
    };
    write.check()?;
    let said = |path: &str| {
        format!(
            "Remembered ({}persistent) → {path}",
            if shared { "shared " } else { "" }
        )
    };
    match crate::brokered::forwarded(write) {
        Forwarded::Written { path, .. } => Ok(said(&path)),
        Forwarded::Refused(why) | Forwarded::Unsure(why) => {
            Err(format!("nothing was written: {why}"))
        }
        Forwarded::NotTaken(not) if !not.may_write_outside() => Err(format!(
            "nothing was written: {}",
            not.refusal("purlis persona remember")
        )),
        Forwarded::NotTaken(_) => {
            let here = crate::Here::read()?;
            let root = here.plane.root();
            if let purlis_core::compat::Compat::ReadOnly(why) = purlis_core::compat::read(root) {
                return Err(format!("nothing was written: {why}"));
            }
            let owner = if shared {
                purlis_core::contain::SHARED_PERSONA.to_owned()
            } else {
                here.active_persona(None).ok_or_else(|| {
                    "this chat runs as no persona, so it has no persona memory of its own: \
                     remember it with shared, or in its workspace with memory_add"
                        .to_owned()
                })?
            };
            let path = purlis_core::brokered::remember_persona(
                root,
                &owner,
                &text,
                title.as_deref(),
                chrono::Local::now().naive_local(),
            )?;
            Ok(said(&crate::voice::rel(root, &path)))
        }
    }
}

/// `workspace_vision` and `workspace_section`: the vision, or one entry of a section, of this
/// chat's workspace's `workspace.md`, which the app that started the chat writes (a brokered
/// write, #1384), as it does [`persona_remember`]'s, on the same terms: this server writes it
/// itself only where no app could have.
fn workspace_write(
    tool: &str,
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    use crate::brokered::Forwarded;
    use purlis_core::brokered::Write;
    let write = chattools::workspace_write(tool, args)?;
    write.check()?;
    let said = |to: &str, path: &str| match &write {
        Write::WorkspaceSection { section, .. } => {
            format!("Added to ## {} of '{to}' → {path}", section.header())
        }
        _ => format!("Vision set for '{to}' → {path}"),
    };
    match crate::brokered::forwarded(write.clone()) {
        Forwarded::Written { to, path } => Ok(said(&to, &path)),
        Forwarded::Refused(why) | Forwarded::Unsure(why) => {
            Err(format!("nothing was written: {why}"))
        }
        Forwarded::NotTaken(not) if !not.may_write_outside() => {
            let instead = match &write {
                Write::WorkspaceVision { .. } => not.refusal("purlis workspace vision \"<text>\""),
                // No command writes a section (D-1384-5, in #1593's "Decided in
                // implementation"), and a chat in a clone is refused the file by its sandbox (ADR
                // 0067 §2), so the edit is not the only way named (#1598).
                _ => format!(
                    "{}: edit workspace.md to add it, or ask the person to",
                    not.why()
                ),
            };
            Err(format!("nothing was written: {instead}"))
        }
        Forwarded::NotTaken(_) => {
            let here = crate::Here::read()?;
            let root = here.plane.root();
            if let purlis_core::compat::Compat::ReadOnly(why) = purlis_core::compat::read(root) {
                return Err(format!("nothing was written: {why}"));
            }
            let place = here.place(None);
            let written = match &write {
                Write::WorkspaceVision { text } => {
                    purlis_core::brokered::set_vision(root, &place, "this chat", text)?
                }
                Write::WorkspaceSection { section, text } => purlis_core::brokered::add_to_section(
                    root,
                    &place,
                    "this chat",
                    *section,
                    text,
                )?,
                _ => return Err(format!("{tool} is not a workspace.md tool")),
            };
            Ok(said(&written.to, &written.path))
        }
    }
}

/// `dispatch`: the one operation `purlis dispatch` run in the chat performs
/// ([`crate::dispatch::send`]), so the app that started the chat decides and starts the persona
/// chat (#1436). Answered with what the command prints; a refusal is a tool error.
///
/// **Only the app starts a chat**, so where this server has no connection to it (it was handed
/// no [`chattools::CONNECTION_ENV`]) nothing is started, and the answer says to run the command
/// in the chat.
fn dispatch(args: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let (to, name, brief) = chattools::dispatch_args(args)?;
    let profile = chattools::dispatch_profile(args)?;
    let waits = chattools::dispatch_waits(args)?;
    let place = chattools::dispatch_place(args)?;
    let long = chattools::dispatch_long(args)?;
    let here = crate::Here::read()?;
    crate::dispatch::send(
        &here,
        to.as_deref(),
        &name,
        &brief,
        &crate::dispatch::Options {
            profile: profile.as_deref(),
            place: place.as_deref(),
            long,
        },
        waits,
    )
    .map_err(|why| in_the_chat(&why))
}

/// `dispatch_read` (#2084): what task `chat` said last and the commands it ran, as
/// `purlis dispatch read` prints it ([`crate::dispatch::read`]).
fn dispatch_read(args: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let chat = chattools::dispatch_read_args(args)?;
    crate::dispatch::read(chat).map_err(|why| in_the_chat(&why))
}

/// `dispatch_report`: the one operation `purlis dispatch report` performs
/// ([`crate::dispatch::report`]).
fn dispatch_report(args: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let (outcome, text, changed) = chattools::dispatch_report_args(args)?;
    crate::dispatch::report(&outcome, &text, changed.as_deref()).map_err(|why| in_the_chat(&why))
}

/// What a tool about the chat's own tasks, or a task's to its asker, runs (#3786): the
/// function the matching `purlis dispatch` subcommand runs, with the tool's arguments.
type TaskTool = fn(&serde_json::Map<String, serde_json::Value>) -> Result<String, String>;

/// The [`TaskTool`] named `name`, or `None` for any other tool.
fn task_tool(name: &str) -> Option<TaskTool> {
    use crate::dispatch;
    use chattools::{dispatch_task, dispatch_text, dispatch_wait_seconds};
    let run: TaskTool = match name {
        chattools::DISPATCH_WAIT => |args| {
            dispatch::wait(
                dispatch_task(args, chattools::DISPATCH_WAIT)?,
                dispatch_wait_seconds(args)?,
            )
        },
        chattools::DISPATCH_TELL => |args| {
            dispatch::tell(
                dispatch_task(args, chattools::DISPATCH_TELL)?,
                &dispatch_text(args, "text", chattools::DISPATCH_TELL)?,
            )
        },
        chattools::DISPATCH_ANSWER => |args| {
            dispatch::answer(
                dispatch_task(args, chattools::DISPATCH_ANSWER)?,
                &dispatch_text(args, "text", chattools::DISPATCH_ANSWER)?,
            )
        },
        chattools::DISPATCH_CANCEL => {
            |args| dispatch::cancel(dispatch_task(args, chattools::DISPATCH_CANCEL)?)
        }
        chattools::DISPATCH_CLOSE => {
            |args| dispatch::close(dispatch_task(args, chattools::DISPATCH_CLOSE)?)
        }
        chattools::DISPATCH_NOTE => {
            |args| dispatch::note(&dispatch_text(args, "text", chattools::DISPATCH_NOTE)?)
        }
        chattools::DISPATCH_ASK => |args| {
            dispatch::ask(
                &dispatch_text(args, "question", chattools::DISPATCH_ASK)?,
                dispatch_wait_seconds(args)?,
            )
        },
        _ => return None,
    };
    Some(run)
}

/// `why`, with where to go when this server was handed no connection to the app.
fn dispatch_tools_have_no_connection() -> bool {
    purlis_core::envvar::var_os(purlis_core::hookwire::SOCKET_ENV)
        .filter(|socket| !socket.is_empty())
        .is_none()
}

fn in_the_chat(why: &str) -> String {
    if dispatch_tools_have_no_connection() {
        format!(
            "{why} This harness hands purlis's tools no connection to the app: run `purlis \
             dispatch` in the chat instead."
        )
    } else {
        why.to_owned()
    }
}

fn tool(spec: &chattools::Tool) -> Tool {
    let schema = match (spec.schema)() {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    let mut annotations = ToolAnnotations::new();
    annotations.read_only_hint = Some(spec.read_only);
    Tool::new(spec.name, spec.description, schema).with_annotations(annotations)
}

/// A tool's answer, or its refusal as an error the model reads.
fn done(answered: Result<String, String>) -> CallToolResponse {
    done_with(answered, Vec::new())
}

/// [`done`], with what the app attached to its answer after the text (#1996): an image as an
/// image, anything else as an embedded resource of its media type, each base64 as MCP carries
/// bytes.
fn done_with(
    answered: Result<String, String>,
    attached: Vec<purlis_core::hookwire::framed::Attached>,
) -> CallToolResponse {
    let (ok, text) = match answered {
        Ok(text) => (true, text),
        Err(why) => (false, why),
    };
    let mut content = vec![ContentBlock::text(text)];
    for (n, one) in attached.into_iter().enumerate() {
        let data = one.base64();
        content.push(if one.media_type.starts_with("image/") {
            ContentBlock::image(data, one.media_type)
        } else {
            ContentBlock::resource(rmcp::model::ResourceContents::BlobResourceContents {
                uri: format!("purlis:attached/{}", n + 1),
                mime_type: Some(one.media_type),
                blob: data,
                meta: None,
            })
        });
    }
    if ok {
        CallToolResult::success(content)
    } else {
        CallToolResult::error(content)
    }
    .into()
}

/// The question, put to the person through the harness's own prompt: an elicitation, which the
/// harness shows to the person and the model never answers. From protocol 2026-07-28 it is asked
/// by returning `input_required` and read from the retried call (SEP-2322); before it, by a
/// request sent while the call is in flight.
///
/// **The wait is said to the app only once the question really goes to the person** ([`Waiting`]):
/// never for a question refused or a harness that cannot ask, and its end on every way out.
async fn ask_operator(
    request: &CallToolRequestParams,
    args: &serde_json::Map<String, serde_json::Value>,
    context: &RequestContext<RoleServer>,
    waiting: &Waiting,
) -> Result<CallToolResponse, ErrorData> {
    let question = match chattools::question(args) {
        Ok(question) => question,
        Err(why) => return Ok(done(Err(why))),
    };
    let state = chattools::question_state(&question);
    if let Some(answer) = request
        .input_responses
        .as_ref()
        .and_then(|responses| responses.get(ANSWER))
    {
        // An answer is read back only for the question it answers.
        if request.request_state.as_deref() != Some(state.as_str()) {
            return Ok(done(Err(
                "this answer was given to another question, so it was not read; ask again"
                    .to_owned(),
            )));
        }
        let answered = serde_json::from_value::<ElicitResult>(answer.clone())
            .map_err(|e| format!("the harness answered in a shape purlis cannot read: {e}"))
            .and_then(said);
        return Ok(done(answered));
    }
    let params = ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: chattools::asked(&question),
        requested_schema: answer_schema(),
    };
    let asks_by_returning = context
        .protocol_version()
        .is_some_and(|v| v.as_str() >= ProtocolVersion::V_2026_07_28.as_str());
    // A 2026-07-28 harness says what it can do on each request; an older one said it once, at
    // `initialize`.
    let can_ask = if asks_by_returning {
        context
            .meta
            .client_capabilities()
            .and_then(|capabilities| capabilities.elicitation)
            .is_some_and(|elicitation| elicitation.form.is_some() || elicitation.url.is_none())
    } else {
        context
            .peer
            .supported_elicitation_modes()
            .contains(&rmcp::service::ElicitationMode::Form)
    };
    if !can_ask {
        return Ok(done(Err(
            "this harness cannot ask the operator through MCP; ask in your reply instead"
                .to_owned(),
        )));
    }
    if asks_by_returning {
        let mut requests = InputRequests::new();
        requests.insert(
            ANSWER.to_owned(),
            InputRequest::Elicitation(rmcp::model::ElicitRequest::new(params)),
        );
        waiting.returned();
        return Ok(InputRequiredResult::new(Some(requests), Some(state)).into());
    }
    // Held until the answer is read, the harness errs, or the call is dropped: each ends the wait.
    let _asked = waiting.asks();
    let answered = context
        .peer
        .create_elicitation_with_timeout(params, None)
        .await
        .map_err(|e| format!("the operator could not be asked: {e}"))
        .and_then(said);
    Ok(done(answered))
}

/// The `notification_type` this server says a question waits on the person under (D-1089-2):
/// what it is, a form an MCP server asked the harness to show, in the word Claude Code says of
/// the same form, which the app reads as `Prompt::Form`. Not `question`: the app takes a
/// question's answer from the chat moving on, never from a notice, so a question said here
/// would stand past its answer.
const ASKS: &str = "elicitation_dialog";

/// The `notification_type` it says the form was answered under, as Claude Code says it of the
/// same form: the app reads it as the form answered, and the chat back at work.
const ANSWERED: &str = "elicitation_complete";

/// **Whether the chat's harness says an `ask_operator` wait itself** (#1089, D-1089-1), so this
/// server must not say it again. Read from the harness the app put into the chat's environment
/// ([`purlis_core::hookwire::HARNESS_ENV`]), the one the permission hook reads too, and never
/// from anything the chat or the harness says over MCP.
///
/// - **Claude Code says it**, as `Notification` `elicitation_dialog` and then
///   `elicitation_complete` for an MCP server's form, so the server is silent there. A chat with
///   no harness named is read as Claude Code, as the permission hook reads it.
/// - **Codex and opencode have no such notice**, so the server says it.
/// - **A harness purlis does not know** is left to say nothing: the question still asks in its
///   pane, and a wait said for a harness unmeasured could be said twice.
fn says_its_own_wait(harness: Option<&str>) -> bool {
    !matches!(
        purlis_core::harness::hooked::Source::of_harness(harness),
        Some(
            purlis_core::harness::hooked::Source::Codex
                | purlis_core::harness::hooked::Source::Opencode
        )
    )
}

/// The report of a question that now waits on the person (`waits`) or no longer does, as the
/// chat's hooks would make it, read from `env`; none where the harness says it itself or no
/// chat of the app's runs this server. It carries no word of the question: only the kind.
fn wait_report(
    waits: bool,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<purlis_core::hookwire::Report> {
    if says_its_own_wait(purlis_core::hookwire::chat_harness(env).as_deref()) {
        return None;
    }
    let kind = if waits { ASKS } else { ANSWERED };
    crate::permission::notification(kind, &serde_json::Value::Null, env)
}

/// What of this server's questions waits on the person now.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Open {
    /// Questions asked by a request in flight (before protocol 2026-07-28), each until its
    /// answer is read or its call ends.
    in_flight: usize,
    /// A question asked by returning `input_required`, until the harness calls again.
    returned: bool,
}

impl Open {
    fn waits(self) -> bool {
        self.in_flight > 0 || self.returned
    }
}

/// **Whether a question of this server's waits on the person, said to the app as it moves**
/// (#1089): the wait once, when the first question goes out, and its end once, when the last is
/// answered or let go.
///
/// **It never says a wait the person does not have.** A chat that could make itself look as if
/// it waits on the person would hold off what purlis does only to a chat that does not (a
/// stop's turn to report, a line typed into its pane), so:
///
/// - the wait is said only as a question goes to the harness to show, never for one refused,
///   nor where the harness cannot ask;
/// - its end is said on every way out: the answer read, the harness erring, the call dropped
///   ([`Asked`]'s drop), the harness's next call after a question it was handed to show, and
///   the session's end ([`Waiting::over`]);
/// - what is said carries no word of the question, and is judged by the app as any hook of the
///   chat's is: its token, and the conversation it names, which a server cannot name for a
///   chat its harness has already spoken in (ADR 0024), so the app takes it only where it may.
struct Waiting {
    open: std::sync::Mutex<Open>,
    /// Says the wait (`true`) or its end (`false`).
    tell: Box<dyn Fn(bool) + Send + Sync>,
}

impl Waiting {
    /// For the chat this server runs in: said on the chat's hook socket, best effort.
    fn of_this_chat() -> Self {
        Self::telling(Box::new(tell_the_app))
    }

    fn telling(tell: Box<dyn Fn(bool) + Send + Sync>) -> Self {
        Self {
            open: std::sync::Mutex::new(Open::default()),
            tell,
        }
    }

    /// `change`, and the app told where it moved whether anything waits.
    fn moved(&self, change: impl FnOnce(&mut Open)) {
        let (was, is) = {
            let mut open = self
                .open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let was = open.waits();
            change(&mut open);
            (was, open.waits())
        };
        if was != is {
            (self.tell)(is);
        }
    }

    /// A question goes to the person on a request in flight, until the guard is dropped.
    fn asks(&self) -> Asked<'_> {
        self.moved(|open| open.in_flight += 1);
        Asked(self)
    }

    /// A question was handed to the harness to show, by returning `input_required`.
    fn returned(&self) {
        self.moved(|open| open.returned = true);
    }

    /// The harness called again: a question it was handed to show is behind it.
    fn heard_again(&self) {
        self.moved(|open| open.returned = false);
    }

    /// The session is over: nothing of it waits.
    fn over(&self) {
        self.moved(|open| *open = Open::default());
    }
}

/// One question in flight; dropping it ends its wait.
struct Asked<'a>(&'a Waiting);

impl Drop for Asked<'_> {
    fn drop(&mut self) {
        self.0
            .moved(|open| open.in_flight = open.in_flight.saturating_sub(1));
    }
}

/// Says the wait or its end on the chat's hook socket, where this server was handed one. A
/// server with no socket says nothing, and the question still asks.
fn tell_the_app(waits: bool) {
    use purlis_core::hookwire;
    let Some(socket) = purlis_core::envvar::var_os(hookwire::SOCKET_ENV).filter(|s| !s.is_empty())
    else {
        return;
    };
    let Some(report) = wait_report(waits, &|name| purlis_core::envvar::var(name)) else {
        return;
    };
    // Best effort, as every report is: the question asks in the pane either way.
    let _ = hookwire::deliver_report(
        std::path::Path::new(&socket),
        hookwire::ChatToken::from_env().as_ref(),
        &report,
    );
}

/// What the operator's answer says, as the chat reads it.
fn said(result: ElicitResult) -> Result<String, String> {
    match result.action {
        ElicitationAction::Accept => result
            .content
            .as_ref()
            .and_then(|content| content.get(ANSWER))
            .and_then(serde_json::Value::as_str)
            .map(|answer| format!("The operator answered: {answer}"))
            .ok_or_else(|| "the operator accepted but gave no answer".to_owned()),
        ElicitationAction::Decline => Ok("The operator declined to answer.".to_owned()),
        ElicitationAction::Cancel => Ok("The operator dismissed the question.".to_owned()),
        _ => Err("the harness answered in a way purlis does not know".to_owned()),
    }
}

/// One line of text, the operator's answer.
fn answer_schema() -> ElicitationSchema {
    serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": { ANSWER: {
            "type": "string",
            "title": "Your answer",
            "description": chattools::NO_SECRET_HERE,
        } },
        "required": [ANSWER],
    }))
    .expect("a fixed schema")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
            .block_on(future)
    }

    #[test]
    fn a_tool_that_panics_answers_an_error_and_never_hangs() {
        let answered = run(blocking(|| panic!("a tool went wrong")));
        let why = answered.expect_err("an error");
        assert!(why.contains("failed inside purlis"), "{why}");
    }

    fn env_of(harness: Option<&'static str>) -> impl Fn(&str) -> Option<String> {
        move |name| match name {
            purlis_core::hookwire::CHAT_ENV => Some("7".to_owned()),
            purlis_core::hookwire::SOCKET_ENV => Some("/tmp/s.sock".to_owned()),
            purlis_core::hookwire::HARNESS_ENV => harness.map(str::to_owned),
            _ => None,
        }
    }

    #[test]
    fn a_harness_with_no_notice_of_its_own_has_the_server_say_the_wait_and_its_end() {
        use purlis_core::harness::model::Prompt;
        use purlis_core::state::{Event, Notified};
        for harness in ["codex", "opencode"] {
            let env = env_of(Some(harness));
            let waits = wait_report(true, &env).expect("the wait is said");
            assert_eq!(waits.chat, 7);
            assert_eq!(waits.event, Event::Notification);
            assert_eq!(waits.detail.notified, Notified::Asks(Prompt::Form));
            assert!(!waits.detail.idle, "{harness}");
            assert_eq!(waits.agent, None, "{harness}");
            let ends = wait_report(false, &env).expect("the end is said");
            assert_eq!(ends.detail.notified, Notified::Answered, "{harness}");
        }
    }

    #[test]
    fn claude_code_says_its_own_wait_so_the_server_says_nothing() {
        // Claude Code reports `elicitation_dialog` and `elicitation_complete` itself; a chat
        // that names no harness is read as Claude Code, and one purlis does not know says
        // nothing it has not measured.
        for harness in [Some("claude-code"), None, Some("aider")] {
            let env = env_of(harness);
            assert!(wait_report(true, &env).is_none(), "{harness:?}");
            assert!(wait_report(false, &env).is_none(), "{harness:?}");
        }
    }

    #[test]
    fn a_server_no_chat_of_the_app_s_runs_says_nothing() {
        let env =
            |name: &str| (name == purlis_core::hookwire::HARNESS_ENV).then(|| "codex".to_owned());
        assert!(wait_report(true, &env).is_none());
    }

    /// A [`Waiting`] that records what it says.
    fn recording() -> (Waiting, std::sync::Arc<std::sync::Mutex<Vec<bool>>>) {
        let said = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let into = std::sync::Arc::clone(&said);
        let waiting = Waiting::telling(Box::new(move |waits| into.lock().unwrap().push(waits)));
        (waiting, said)
    }

    #[test]
    fn a_question_in_flight_is_said_once_and_ended_when_its_call_ends() {
        let (waiting, said) = recording();
        {
            let _one = waiting.asks();
            let _two = waiting.asks();
            assert_eq!(*said.lock().unwrap(), [true]);
        }
        // Both answered: the end once, and nothing more on a later call.
        waiting.heard_again();
        waiting.over();
        assert_eq!(*said.lock().unwrap(), [true, false]);
    }

    #[test]
    fn a_question_in_flight_whose_call_is_dropped_still_ends_its_wait() {
        let (waiting, said) = recording();
        // A call's future dropped mid-wait (the harness cancelled it) drops its guard.
        let pending = {
            let waiting = &waiting;
            async move {
                let _asked = waiting.asks();
                std::future::pending::<()>().await;
            }
        };
        let mut pending = Box::pin(pending);
        let waker = std::task::Waker::noop();
        let _ = pending
            .as_mut()
            .poll(&mut std::task::Context::from_waker(waker));
        assert_eq!(*said.lock().unwrap(), [true]);
        drop(pending);
        assert_eq!(*said.lock().unwrap(), [true, false]);
    }

    #[test]
    fn a_question_handed_to_the_harness_ends_at_its_next_call_or_the_session_s_end() {
        let (waiting, said) = recording();
        waiting.returned();
        waiting.returned();
        waiting.heard_again();
        assert_eq!(*said.lock().unwrap(), [true, false]);

        waiting.returned();
        waiting.over();
        assert_eq!(*said.lock().unwrap(), [true, false, true, false]);
    }

    #[test]
    fn nothing_is_said_where_nothing_waits() {
        let (waiting, said) = recording();
        waiting.heard_again();
        waiting.over();
        assert!(said.lock().unwrap().is_empty());
    }

    #[test]
    fn a_tool_that_returns_is_answered_as_it_returned() {
        assert_eq!(
            run(blocking(|| Ok("done".to_owned()))),
            Ok("done".to_owned())
        );
    }

    #[test]
    fn what_the_app_attaches_reaches_the_model_after_the_text_an_image_as_an_image() {
        use purlis_core::hookwire::framed::Attached;
        let CallToolResponse::Complete(result) = done_with(
            Ok("where it works".to_owned()),
            vec![
                Attached {
                    media_type: "image/png".to_owned(),
                    bytes: vec![0, 255, 10],
                },
                Attached {
                    media_type: "application/pdf".to_owned(),
                    bytes: vec![1],
                },
            ],
        ) else {
            panic!("a complete answer");
        };
        assert_eq!(result.is_error, Some(false));
        let content = serde_json::to_value(&result.content).expect("JSON");
        assert_eq!(content[0]["text"], "where it works");
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["data"], "AP8K");
        assert_eq!(content[1]["mimeType"], "image/png");
        assert_eq!(content[2]["type"], "resource");
        assert_eq!(content[2]["resource"]["blob"], "AQ==");
        assert_eq!(content[2]["resource"]["mimeType"], "application/pdf");
    }
}
