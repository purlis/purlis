// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! charter's core, shared by the desktop app and the `charter` binary.

pub mod acp;
pub mod active;
pub mod activity;
pub mod adopt;
pub mod alerts;
pub mod applog;
pub mod askoutcome;
pub mod autosave;
pub mod awareness;
pub mod briefing;
pub mod brokered;
pub mod browsertools;
pub mod change;
pub mod chatenv;
pub mod chatpiece;
pub mod chatspec;
pub mod chattools;
pub mod cistate;
pub mod cliname;
pub mod clipath;
pub mod cloneupdate;
pub mod codelink;
pub mod commitgate;
pub mod commitguard;
pub mod committedby;
pub mod compat;
pub mod consentspelling;
pub mod contain;
pub mod counts;
pub mod createproject;
pub mod credguard;
pub mod curate;
pub mod curation;
pub mod datahome;
pub mod didnotstart;
pub mod diffscan;
pub mod dispatch;
pub mod dispatcharrival;
pub mod dispatchaway;
pub mod dispatchchain;
pub mod dispatchdecision;
pub mod dispatchdormant;
pub mod dispatched;
pub mod dispatchgrant;
pub mod dispatchguard;
pub mod dispatchlimits;
pub mod dispatchnever;
pub mod dispatchplace;
pub mod dispatchprofiles;
pub mod dispatchrecord;
pub mod dispatchrestart;
pub mod dispatchtalk;
pub mod dispatchunattended;
pub mod dispatchwants;
pub mod dispatchwithin;
#[cfg(unix)]
pub mod dockerpool;
pub mod docsrc;
pub mod doctor;
pub mod doing;
/// The terminal engine, from the crate that holds it without the rest of the core (#1995).
pub use purlis_terminal::engine;
pub mod browserlimits;
#[cfg(unix)]
pub mod browserlogins;
pub mod browsers;
pub mod browsersatquit;
pub mod envvar;
pub mod eventlog;
pub mod executor;
pub mod extension;
pub mod fence;
pub mod filelock;
pub mod files;
pub mod firstrun;
pub mod firsttask;
pub mod floorguard;
pub mod footer;
pub mod footerclaim;
pub mod forge;
pub mod forklock;
pub mod getproject;
pub mod gitbroker;
pub mod gitconfig;
pub mod githooks;
pub mod gitpolicy;
pub mod gitstate;
pub mod glrefresh;
pub mod glstate;
pub mod guardcaps;
pub mod guardcmd;
pub mod guardseen;
pub mod guest;
pub mod halt;
pub mod handback;
pub mod handed;
pub mod handoff;
pub mod handoffguard;
pub mod harness;
pub mod harness_card;
pub mod harness_declaration;
pub mod harness_plugin;
#[cfg(unix)]
pub(crate) mod held;
pub mod heredoc;
#[cfg(unix)]
pub mod hookchannel;
pub mod hookreg;
pub mod hookstate;
pub mod hookwire;
pub mod hostsocket;
pub mod hoststart;
pub mod inboxupdates;
pub mod instructions;
pub mod inventory;
pub mod layer;
pub mod leakguard;
pub mod leftovers;
#[cfg(test)]
mod lineartime;
#[cfg(unix)]
pub mod linkkey;
pub mod livesub;
pub mod localproject;
pub mod machine;
pub mod manifest;
pub mod mdsection;
pub mod memorypressure;
pub mod memscope;
pub mod memstore;
pub mod names;
pub mod netlog;
pub mod news;
pub mod noharness;
pub mod noterminal;
pub mod onboarding;
pub mod opencode;
pub mod openfiles;
pub mod panel;
pub mod personacmd;
pub mod personagate;
pub mod personagrant;
pub mod personamark;
pub mod personaprofile;
pub mod personas;
pub mod personaverbs;
pub mod piececmd;
pub mod pieces;
pub mod pieceupdate;
pub mod plane;
pub mod planechange;
pub mod planegit;
pub mod planemodel;
pub mod planeroot;
pub mod planesave;
pub mod plugin;
pub mod plugin_install;
#[cfg(unix)]
pub mod pools;
pub mod process;
pub mod profiles;
pub mod profilesignin;
pub mod profilestore;
pub mod profiletrust;
pub mod programs;
pub mod projectgit;
pub mod proposed;
pub mod proseguard;
pub mod provenance;
pub mod publishproject;
pub mod pyjson;
pub mod pypath;
pub mod pyrepr;
pub mod recall;
pub mod reference;
pub mod removalask;
pub mod renamelocal;
pub mod reopen;
pub mod repocmd;
pub mod repoinstructions;
pub mod repolist;
pub mod report;
pub mod repos;
pub mod reposave;
pub mod restartends;
pub mod retention;
pub mod rewrite;
pub mod rootupdate;
pub mod roster;
pub mod rulespelling;
#[cfg(unix)]
pub mod runnerimage;
#[cfg(unix)]
pub mod runnerlink;
#[cfg(unix)]
pub mod runners;
pub mod runnerwrite;
pub mod sandbox;
pub mod sandboxblock;
pub mod savefix;
pub mod scaffold;
pub mod scanallow;
pub mod secrets;
pub mod secretshape;
/// A program in its own pseudo-terminal, from the crate that holds it without the rest of the
/// core (#1995).
pub use purlis_terminal::session;
pub mod sessionrecord;
pub mod sessionresume;
pub mod settings;
pub mod shellguard;
pub mod shellseg;
pub mod shellsubst;
pub mod shellwrap;
pub mod shown;
pub mod silence;
pub mod skills;
pub mod skillspan;
pub mod skilltools;
pub mod skilluse;
pub mod staleclone;
pub mod standings;
pub mod start;
pub mod state;
pub(crate) mod steer;
pub mod taskchanges;
pub mod template;
// Public, and not `cfg(test)`: the integration tests of every crate copy the fixture planes
// through it (#2089). Nothing in the product calls it.
#[doc(hidden)]
pub mod testfixture;
#[cfg(test)]
mod testgit;
#[cfg(all(test, unix))]
mod testmulticall;
// Public, and not `cfg(test)`, for one reason: purlis-core's integration tests are separate
// crates, and they re-run themselves through it (charter-app#267). Nothing in the product
// calls it.
#[doc(hidden)]
pub mod testrun;
pub mod theme;
pub mod themechat;
pub mod themepreview;
pub mod toolgate;
pub mod toolhooks;
pub mod touching;
pub mod trace;
pub mod tui;
pub mod updates;
pub mod usage;
pub mod version;
#[cfg(unix)]
pub mod windownames;
pub mod windowprefs;
pub mod wiring;
pub mod work;
pub mod workspaces;
pub mod worktree;
pub mod wscmd;
pub mod wslayer;
pub mod youreditor;
