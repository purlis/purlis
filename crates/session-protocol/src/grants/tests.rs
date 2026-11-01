// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn only_local_ui_reaches_the_ui_rpc_or_answers_a_secrets_ask() {
    for scope in Scope::ALL {
        assert_eq!(scope.may(Power::UiRpc), scope == Scope::LocalUi, "{scope}");
        assert_eq!(
            scope.may(Power::AnswerASecret),
            scope == Scope::LocalUi,
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_or_its_hosts_relay_hands_a_host_a_composed_launch_or_holds_a_project() {
    for scope in Scope::ALL {
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(scope.may(Power::Launch), windows, "{scope}");
        assert_eq!(scope.may(Power::Hold), windows, "{scope}");
    }
}

#[test]
fn only_the_window_or_its_hosts_relay_replies_to_what_a_projects_hooks_wrote() {
    for scope in Scope::ALL {
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(scope.may(Power::Heard), windows, "{scope}");
    }
}

#[test]
fn only_the_window_or_its_hosts_relay_confines_a_chat_and_answers_its_sandbox() {
    for scope in Scope::ALL {
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(scope.may(Power::Confine), windows, "{scope}");
    }
}

#[test]
fn only_the_window_hands_its_own_host_a_chat_it_composed_for_a_harness() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::LaunchAHarness),
            scope == Scope::LocalUi,
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_reads_or_moves_a_projects_board() {
    for scope in Scope::ALL {
        assert_eq!(scope.may(Power::Board), scope == Scope::LocalUi, "{scope}");
    }
}

#[test]
fn only_the_window_takes_back_a_projects_programs_or_quits_its_host() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::Reattach),
            scope == Scope::LocalUi,
            "{scope}"
        );
        assert_eq!(scope.may(Power::Quit), scope == Scope::LocalUi, "{scope}");
    }
}

#[test]
fn only_the_window_and_a_link_between_hosts_ask_a_projects_programs() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::ProgramsOf),
            matches!(scope, Scope::LocalUi | Scope::RemoteLink),
            "{scope}"
        );
    }
}

#[test]
fn only_a_relaying_host_takes_back_its_windows_runner_chats() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::TakeBack),
            scope == Scope::RemoteLink,
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_names_itself_to_its_host() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::NameWindow),
            scope == Scope::LocalUi,
            "{scope}"
        );
    }
}

#[test]
fn only_a_desktop_on_its_link_to_a_runner_unpairs() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::Unpair),
            scope == Scope::RemoteLink,
            "{scope}"
        );
    }
}

#[test]
fn only_a_desktop_on_its_link_to_a_runner_has_it_keep_a_profile() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::KeepProfile),
            scope == Scope::RemoteLink,
            "{scope}"
        );
    }
}

#[test]
fn only_a_desktop_on_its_link_to_a_runner_hands_it_a_sign_ins_redirect() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::SignInRedirect),
            scope == Scope::RemoteLink,
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_or_its_hosts_relay_starts_watches_or_logs_in_a_browser() {
    for scope in Scope::ALL {
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(scope.may(Power::Browser), windows, "{scope}");
        assert_eq!(scope.may(Power::LogIn), windows, "{scope}");
    }
}

#[test]
fn only_the_window_opens_or_keeps_a_kept_login() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::KeptLogin),
            scope == Scope::LocalUi,
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_writes_in_the_event_log_the_host_holds() {
    for scope in Scope::ALL {
        assert_eq!(scope.may(Power::Record), scope == Scope::LocalUi, "{scope}");
    }
}

#[test]
fn only_the_window_saves_a_held_project() {
    for scope in Scope::ALL {
        assert_eq!(scope.may(Power::Save), scope == Scope::LocalUi, "{scope}");
    }
}

#[test]
fn only_the_window_opts_a_chat_out_of_the_sandbox() {
    for scope in Scope::ALL {
        assert_eq!(scope.may(Power::OptOut), scope == Scope::LocalUi, "{scope}");
    }
}

#[test]
fn only_a_human_scope_answers() {
    let answer: Vec<Scope> = Scope::ALL
        .into_iter()
        .filter(|scope| scope.may(Power::Answer))
        .collect();
    assert_eq!(answer, [Scope::LocalUi, Scope::Approval]);
}

#[test]
fn only_the_window_asks_its_host_to_relay_its_link_to_a_runner() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::RelayTo),
            scope == Scope::LocalUi,
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_or_its_hosts_relay_moves_code_over_the_link() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::Code),
            matches!(scope, Scope::LocalUi | Scope::RemoteLink),
            "{scope}"
        );
    }
}

#[test]
fn only_the_window_re_arms_or_its_host_re_arming_a_runner_it_stopped() {
    for scope in Scope::ALL {
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(scope.may(Power::Rearm), windows, "{scope}");
    }
}

#[test]
fn only_the_window_or_its_hosts_relay_reads_a_hosts_own_event_log() {
    for scope in Scope::ALL {
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(scope.may(Power::EventLog), windows, "{scope}");
    }
}

#[test]
fn only_a_terminal_asks_for_a_grade_and_only_the_window_runs_one() {
    for scope in Scope::ALL {
        assert_eq!(
            scope.may(Power::AskGrade),
            scope == Scope::Terminal,
            "{scope}"
        );
        assert_eq!(
            scope.may(Power::RunGrade),
            scope == Scope::LocalUi,
            "{scope}"
        );
    }
}
