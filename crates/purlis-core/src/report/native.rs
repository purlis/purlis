// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A native crash, from the crash handler's minidump to a Crash report** (#5691, spec #5683,
//! ruling O9).
//!
//! A panic hook cannot see a segfault, a fault on a stack's guard page or an uncaught
//! Objective-C exception: the process is gone first. The app's crash handler, a companion process
//! (`purlis-crash-handler`), writes a minidump of it from outside into [`crashes_dir`], named
//! `<seconds since 1970>-<id>.dmp`. On the next launch the app calls [`report_waiting`], which
//! takes the crashing thread's stack out of each ([`stack_of`]) and hands that to [`Reports`] as
//! [`Evidence::NativeCrash`]: with a yes to Crash reports it goes to the Report intake, and only
//! there.
//!
//! **Only the crashing thread's stack leaves the device** (the operator's ruling (b) of
//! 2026-11-10, on #5691): its frames as module names and offsets, with each module's build id for
//! the intake to find its symbols by, and the crash's kind. No memory contents, no other thread,
//! no environment. The minidump itself never leaves: once reported, it is kept here as
//! `<seconds since 1970>-<id>.kept` until it is older than [`KEPT_FOR_SECS`].
//!
//! **Tier: Machine, device-bound** (ADR 0069): `<config>/crashes/`, made `0700` by the handler.

use std::path::{Path, PathBuf};

use minidump::{
    Minidump, MinidumpException, MinidumpMiscInfo, MinidumpModuleList, MinidumpSystemInfo,
    MinidumpThreadList, Module as _,
};
use minidump_unwind::{CallStack, FrameTrust, MultiSymbolProvider};

use crate::forge::Caller;

use super::{Evidence, Outcome, Reports};

/// A native crash: when it was, and its crashing thread's stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCrash {
    /// When it crashed, in seconds since 1970, as the minidump's name says.
    pub at: Option<u64>,
    /// The crashing thread's stack, as the minidump had it: empty when it could not be read.
    pub stack: CrashStack,
}

/// **The crashing thread's stack**: all of a native crash that leaves the device (ruling (b)).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrashStack {
    /// What the crash was, as the system named it (`EXC_BAD_ACCESS / KERN_INVALID_ADDRESS`).
    pub reason: Option<String>,
    /// The processor's architecture, as Sentry names it (`arm64`, `x86_64`).
    pub arch: Option<String>,
    /// The frames, the one that crashed first. Only those within a module.
    pub frames: Vec<Frame>,
    /// The modules the frames are in, each once.
    pub modules: Vec<Module>,
}

/// One frame of a crashed thread: where in which module it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Which of [`CrashStack::modules`] it is in.
    pub module: usize,
    /// How far into that module, in bytes.
    pub offset: u64,
    /// How the frame was found, as Sentry names it (`context`, `fp`, `cfi`, `scan`).
    pub trust: &'static str,
}

/// A module a frame is in: its file's name (never its folder), and what the intake finds its
/// symbols by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    /// Its debug id (a Mach-O UUID, an ELF build id, a PDB's signature and age).
    pub debug_id: Option<String>,
    pub code_id: Option<String>,
    pub size: u64,
}

/// At most this many frames are kept: a crash's place is in its first ones, and a stack that
/// overflowed is thousands deep.
const FRAMES_AT_MOST: usize = 64;

/// How long a minidump is kept on this device (spec #5683, Q24): one older is removed, reported
/// or not.
pub const KEPT_FOR_SECS: u64 = 14 * 24 * 60 * 60;

/// How old a `.part` the companion left is before it is taken for one it will never finish.
const UNFINISHED_AFTER_SECS: u64 = 60 * 60;

/// The folder the crash handler keeps minidumps in: `crashes/` in this machine's config home.
pub fn crashes_dir(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join("crashes")
}

/// **The crashing thread's stack, out of `minidump`**, walked on this device with no symbols
/// (frame pointers, and the stack scanned where they stop), each frame as its module and its
/// offset in it. Nothing else of the minidump is kept: no memory, no other thread, no register.
/// An error, in words, when it holds no crash or cannot be read.
pub fn stack_of(minidump: &[u8]) -> Result<CrashStack, String> {
    let dump = Minidump::read(minidump).map_err(|why| format!("not a minidump: {why}"))?;
    let system = dump
        .get_stream::<MinidumpSystemInfo>()
        .map_err(|why| format!("no system info: {why}"))?;
    let exception = dump
        .get_stream::<MinidumpException>()
        .map_err(|why| format!("no crash in it: {why}"))?;
    let threads = dump
        .get_stream::<MinidumpThreadList>()
        .map_err(|why| format!("no threads: {why}"))?;
    let modules = dump.get_stream::<MinidumpModuleList>().unwrap_or_default();
    let misc = dump.get_stream::<MinidumpMiscInfo>().ok();
    let memory = dump.get_memory().unwrap_or_default();

    let crashed = exception.get_crashing_thread_id();
    let thread = threads
        .get_thread(crashed)
        .ok_or_else(|| format!("the crashing thread {crashed} is not in it"))?;
    let context = exception
        .context(&system, misc.as_ref())
        .or_else(|| thread.context(&system, misc.as_ref()))
        .ok_or("the crashing thread has no registers")?;
    let mut stack = CallStack::with_context(context.into_owned());
    // The memory the stack pointer is in, which may not be the thread's own when it overflowed.
    let at = stack.frames[0].context.get_stack_pointer();
    let stack_memory = thread
        .stack_memory(&memory)
        .filter(|own| own.get_memory_at_address::<u64>(at).is_some())
        .or_else(|| memory.memory_at_address(at))
        .or_else(|| thread.stack_memory(&memory));
    let info = minidump_unwind::SystemInfo {
        os: system.os,
        os_version: None,
        os_build: None,
        cpu: system.cpu,
        cpu_info: None,
        cpu_microcode_version: None,
        cpu_count: 1,
    };
    futures::executor::block_on(minidump_unwind::walk_stack(
        0,
        (),
        &mut stack,
        stack_memory,
        &modules,
        &info,
        &MultiSymbolProvider::new(),
    ));

    let mut kept = CrashStack {
        reason: Some(
            exception
                .get_crash_reason(system.os, system.cpu)
                .to_string(),
        ),
        arch: arch(system.cpu),
        ..CrashStack::default()
    };
    // A signal is answered on the crashing thread itself, so its live registers, all the
    // minidump has, are the handler's: the crash is past the trampoline it was called from.
    let from = if arrived_as_a_signal(&exception, system.os) {
        past_the_trampoline(stack.frames.iter().map(|frame| {
            frame
                .module
                .as_ref()
                .map(|module| file_name(&module.code_file()))
        }))
    } else {
        0
    };
    let mut bases = Vec::new();
    for frame in stack.frames.iter().skip(from) {
        let Some(module) = &frame.module else {
            continue;
        };
        let base = module.base_address();
        let index = bases.iter().position(|b| *b == base).unwrap_or_else(|| {
            bases.push(base);
            kept.modules.push(Module {
                name: file_name(&module.code_file()),
                debug_id: module.debug_identifier().map(|id| id.to_string()),
                code_id: module.code_identifier().map(|id| id.to_string()),
                size: module.size(),
            });
            bases.len() - 1
        });
        kept.frames.push(Frame {
            module: index,
            offset: frame.instruction.saturating_sub(base),
            trust: trust(frame.trust),
        });
        if kept.frames.len() == FRAMES_AT_MOST {
            break;
        }
    }
    Ok(kept)
}

/// Mach's kind for a software exception, `EXC_SOFTWARE`, and its code for one that is a Unix
/// signal, `EXC_SOFT_SIGNAL` (`<mach/exception_types.h>`, `<sys/ux_exception.h>`).
const EXC_SOFTWARE: u32 = 5;
const EXC_SOFT_SIGNAL: u32 = 0x10003;

/// The module macOS's signal trampoline, `_sigtramp`, is in.
const TRAMPOLINE: &str = "libsystem_platform.dylib";

/// Whether the crash arrived as a signal (an abort's `SIGABRT`), which the crash handler hears in
/// a signal handler on the crashing thread and passes on as `EXC_SOFTWARE / EXC_SOFT_SIGNAL`
/// (#5691). A Mach exception (`EXC_BAD_ACCESS`) is heard on the handler's own thread, and the
/// crashing thread's registers are where it crashed.
///
/// minidump-writer records that thread's registers as they are when it writes, not the signal's
/// context (crash-handler does not pass that on), so the stack it walks starts in the handler.
fn arrived_as_a_signal(exception: &MinidumpException, os: minidump::system_info::Os) -> bool {
    let record = &exception.raw.exception_record;
    os == minidump::system_info::Os::MacOs
        && record.exception_code == EXC_SOFTWARE
        && record.exception_flags == EXC_SOFT_SIGNAL
}

/// **How many of a signalled thread's frames are the crash handler's**, each frame as the file
/// name of the module it is in, the innermost first: every frame up to and including the first
/// in [`TRAMPOLINE`], `_sigtramp`, the frame the kernel called the handler from. The frame
/// after it is the one the signal interrupted. With no symbols on the device, the trampoline's
/// module is the marker: the handler waits in `libsystem_kernel` and `libsystem_pthread` and runs
/// in the app's own binary, never in `libsystem_platform`. With no trampoline, none: the stack
/// is kept whole rather than guessed at.
fn past_the_trampoline(modules: impl IntoIterator<Item = Option<String>>) -> usize {
    modules
        .into_iter()
        .position(|module| module.as_deref() == Some(TRAMPOLINE))
        .map_or(0, |trampoline| trampoline + 1)
}

/// A module's file name, without the folders it is in: they may name the person's.
fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// The architecture as Sentry names it.
fn arch(cpu: minidump::system_info::Cpu) -> Option<String> {
    use minidump::system_info::Cpu;
    match cpu {
        Cpu::Arm64 => Some("arm64"),
        Cpu::X86_64 => Some("x86_64"),
        Cpu::X86 => Some("x86"),
        Cpu::Arm => Some("arm"),
        _ => None,
    }
    .map(str::to_string)
}

/// How a frame was found, as Sentry names it.
fn trust(trust: FrameTrust) -> &'static str {
    match trust {
        FrameTrust::Context => "context",
        FrameTrust::CallFrameInfo => "cfi",
        FrameTrust::FramePointer => "fp",
        FrameTrust::CfiScan => "cfi_scan",
        FrameTrust::Scan => "scan",
        FrameTrust::PreWalked => "prewalked",
        FrameTrust::None => "none",
    }
}

/// Hands every minidump waiting in `crashes` to `reports`, oldest first, as a Crash report of its
/// crashing thread's stack from a trigger (nobody is at the keyboard), and answers with each
/// outcome. Each is reported once, whatever the outcome (a report the intake gave no answer for
/// waits in the sent list, #5693): it is then kept here as `.kept`, never sent, until it is older
/// than [`KEPT_FOR_SECS`], when it is removed, as one never reported is. A file still being
/// written (a `.part` of the last hour, or one in a folder) is left alone; an older `.part` is
/// removed unread.
pub fn report_waiting(reports: &Reports, crashes: &Path) -> Vec<Outcome> {
    let now = chrono::Utc::now().timestamp().unsigned_abs();
    let stale = |at: Option<u64>| at.is_none_or(|at| now.saturating_sub(at) > KEPT_FOR_SECS);
    for (path, at) in in_folder(crashes, "kept") {
        if stale(at) {
            let _ = std::fs::remove_file(&path);
        }
    }
    // A dump the companion did not finish (it was killed, or the machine went down) is no
    // crash to report, but holds what it wrote of the process: it goes once nothing can be
    // writing it.
    for (path, at) in in_folder(crashes, "part") {
        if at.is_none_or(|at| now.saturating_sub(at) > UNFINISHED_AFTER_SECS) {
            let _ = std::fs::remove_file(&path);
        }
    }
    let mut outcomes = Vec::new();
    for (path, at) in in_folder(crashes, "dmp") {
        if stale(at) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        let Ok(minidump) = std::fs::read(&path) else {
            continue;
        };
        // One that cannot be read still says the app crashed, and when.
        let stack = stack_of(&minidump).unwrap_or_default();
        drop(minidump);
        let said = reports.take(
            Evidence::NativeCrash(NativeCrash { at, stack }),
            &Caller::trigger(),
        );
        if std::fs::rename(&path, path.with_extension("kept")).is_err() {
            // Never reported twice: a minidump that cannot be set aside goes.
            let _ = std::fs::remove_file(&path);
        }
        outcomes.push(said);
    }
    outcomes
}

/// The files in `crashes` with the `extension`, oldest first, each with when it crashed as its
/// name says.
fn in_folder(crashes: &Path, extension: &str) -> Vec<(PathBuf, Option<u64>)> {
    let Ok(entries) = std::fs::read_dir(crashes) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, Option<u64>)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .map(|path| {
            let at = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.split('-').next())
                .and_then(|secs| secs.parse().ok());
            (path, at)
        })
        .collect();
    found.sort_by_key(|(_, at)| *at);
    found
}

#[cfg(test)]
mod tests {
    use super::past_the_trampoline;

    fn modules(names: &[&str]) -> Vec<Option<String>> {
        names
            .iter()
            .map(|name| (!name.is_empty()).then(|| (*name).to_string()))
            .collect()
    }

    #[test]
    fn an_aborts_stack_starts_past_the_signal_trampoline() {
        // As Sentry event 165c0c11ac34479ca12763e401ace1aa had it, innermost first (#5691).
        let stack = modules(&[
            "libsystem_kernel.dylib",   // __psynch_cvwait
            "purlis-app",               // parking_lot's condvar, in the handler
            "purlis-app",               // HandlerInner::send_message
            "purlis-app",               // simulate_exception
            "purlis-app",               // signal_handler
            "",                         // a frame in no module
            "libsystem_platform.dylib", // _sigtramp
            "libsystem_pthread.dylib",  // pthread_kill
            "libsystem_c.dylib",        // abort
            "purlis-app",               // std::process::abort
            "purlis-app",               // nativecrash::crash_if_asked: the crash
        ]);

        assert_eq!(past_the_trampoline(stack), 7);
    }

    #[test]
    fn a_stack_with_no_signal_trampoline_is_kept_whole() {
        let stack = modules(&["purlis-app", "AppKit", "libdyld.dylib"]);

        assert_eq!(past_the_trampoline(stack), 0);
    }

    #[test]
    fn only_the_first_trampoline_is_the_handlers() {
        // A signal raised in a signal handler: the crash is the inner one's.
        let stack = modules(&[
            "purlis-app",
            "libsystem_platform.dylib",
            "purlis-app",
            "libsystem_platform.dylib",
            "purlis-app",
        ]);

        assert_eq!(past_the_trampoline(stack), 2);
    }
}
