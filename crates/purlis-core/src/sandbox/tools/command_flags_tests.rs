// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Run grant is offered for no command whose flags name a command a program then runs
//! (#5799), and the usual commands of the same programs stay offered.

use super::command_flags::names_a_command;

fn words(command: &str) -> Vec<String> {
    command.split(' ').map(str::to_owned).collect()
}

/// Each of `commands`, `program` and its words split at each space, is refused, saying `form`.
fn refused(program: &str, form: &str, commands: &[&[&str]]) {
    for command in commands {
        let args: Vec<String> = command.iter().map(|one| (*one).to_owned()).collect();
        let why = names_a_command(program, &args)
            .unwrap_or_else(|| panic!("{program} {command:?} was offered"));
        assert!(why.contains(form), "{program} {command:?}: {why}");
        assert!(
            why.contains("declare what it's for as a tool"),
            "{program} {command:?}: {why}"
        );
    }
}

/// Each of `commands` is offered, as `program`'s usual command.
fn offered(program: &str, commands: &[&str]) {
    for command in commands {
        assert_eq!(
            names_a_command(program, &words(command)),
            None,
            "{program} {command}"
        );
    }
}

#[test]
fn ssh_with_an_option_that_runs_a_command_is_offered_no_run_grant() {
    refused(
        "ssh",
        "-o ProxyCommand",
        &[
            &["-oProxyCommand=sh -c x", "host"],
            &["-o", "ProxyCommand=sh", "host"],
            &["-o", "proxycommand sh", "host"],
            &["-4o", "ProxyCommand=sh", "host"],
            &["host", "-o", "ProxyCommand=sh"],
        ],
    );
    refused("ssh", "-o LocalCommand", &[&["-oLocalCommand=x", "host"]]);
    refused(
        "ssh",
        "-o PermitLocalCommand",
        &[&["-o", "PermitLocalCommand=yes", "host"]],
    );
    refused(
        "ssh",
        "-o KnownHostsCommand",
        &[&["-oKnownHostsCommand=x", "host"]],
    );
    refused(
        "ssh",
        "-o PKCS11Provider",
        &[&["-o", "PKCS11Provider=/tmp/x.so", "host"]],
    );
    refused(
        "slogin",
        "-o ProxyCommand",
        &[&["-oProxyCommand=x", "host"]],
    );
}

#[test]
fn ssh_with_a_config_file_or_a_library_it_names_is_offered_no_run_grant() {
    refused("ssh", "-F", &[&["-F", "conf", "host"], &["-Fconf", "host"]]);
    refused("ssh", "-I", &[&["-I", "/tmp/pkcs11.so", "host"]]);
}

#[test]
fn ssh_scp_and_rsync_with_their_usual_flags_stay_offered() {
    offered(
        "ssh",
        &[
            "-i key host uptime",
            "-p 2222 -o StrictHostKeyChecking=no host uptime",
            "-l deploy host",
            "-J bastion host",
        ],
    );
    offered("scp", &["-P 2222 -i key a host:b", "-r dir host:dir"]);
    offered("sftp", &["-P 2222 -i key host"]);
    offered(
        "rsync",
        &[
            "-avz src host:dst",
            "-a --delete --exclude=.git src host:dst",
            "-f -_*.o src dst",
        ],
    );
}

#[test]
fn scp_and_sftp_with_a_program_they_run_are_offered_no_run_grant() {
    refused(
        "scp",
        "-S",
        &[&["-S", "x", "a", "host:b"], &["-Sx", "a", "host:b"]],
    );
    refused("scp", "-D", &[&["-D", "/tmp/sftp-server", "a", "host:b"]]);
    refused("scp", "-F", &[&["-F", "conf", "a", "host:b"]]);
    refused(
        "scp",
        "-o ProxyCommand",
        &[&["-o", "ProxyCommand=x", "a", "host:b"]],
    );
    refused("sftp", "-S", &[&["-S", "x", "host"]]);
    refused("sftp", "-D", &[&["-D", "/tmp/sftp-server", "host"]]);
    refused("sftp", "-F", &[&["-F", "conf", "host"]]);
    refused("sftp", "-b", &[&["-b", "batch", "host"]]);
    refused("sftp", "-o ProxyCommand", &[&["-oProxyCommand=x", "host"]]);
}

#[test]
fn rsync_with_a_remote_shell_or_program_it_names_is_offered_no_run_grant() {
    refused(
        "rsync",
        "-e",
        &[
            &["-e", "x", "a", "host:b"],
            &["-avze", "ssh -p 2", "a", "host:b"],
        ],
    );
    refused(
        "rsync",
        "--rsh",
        &[
            &["--rsh=x", "a", "host:b"],
            &["--rsh", "x", "a", "host:b"],
            &["--rs=x", "a", "host:b"],
        ],
    );
    refused(
        "rsync",
        "--rsync-path",
        &[&["--rsync-path=x", "a", "host:b"]],
    );
    refused("rsync", "-M", &[&["-M--rsh=x", "a", "host:b"]]);
    refused(
        "rsync",
        "--remote-option",
        &[&["--remote-option=--rsh=x", "a", "host:b"]],
    );
    refused("rsync", "--daemon", &[&["--daemon"]]);
    refused("rsync", "--config", &[&["--config=x"]]);
}

#[test]
fn zip_with_the_unzip_command_it_names_is_offered_no_run_grant() {
    refused(
        "zip",
        "-TT",
        &[
            &["-T", "-TT", "x", "a.zip", "f"],
            &["-TTx", "a.zip", "f"],
            &["-qTT", "x", "a.zip", "f"],
        ],
    );
    refused(
        "zip",
        "--unzip-command",
        &[
            &["--unzip-command", "x", "a.zip", "f"],
            &["--unzip-command=x", "a.zip", "f"],
            &["--unz=x", "a.zip", "f"],
        ],
    );
    offered(
        "zip",
        &["-P secret -r a.zip dir", "-T a.zip f", "-e a.zip f"],
    );
}

#[test]
fn wget_with_a_command_it_runs_is_offered_no_run_grant() {
    refused(
        "wget",
        "--use-askpass",
        &[
            &["--use-askpass=x", "https://example.com"],
            &["--use-askpass", "https://example.com"],
        ],
    );
    refused(
        "wget",
        "-e",
        &[
            &["-e", "use_askpass=x", "https://example.com"],
            &["-euse_askpass=x", "https://example.com"],
            &["-qe", "x", "https://example.com"],
        ],
    );
    refused(
        "wget",
        "--execute",
        &[&["--execute", "x", "https://example.com"]],
    );
    refused(
        "wget",
        "--config",
        &[&["--config=x", "https://example.com"]],
    );
    offered(
        "wget",
        &[
            "-q -O out https://example.com",
            "-nv --header=X-Token:1 https://example.com",
        ],
    );
}

#[test]
fn kubectl_with_a_kubeconfig_or_a_plugin_it_names_is_offered_no_run_grant() {
    refused(
        "kubectl",
        "--kubeconfig",
        &[
            &["--kubeconfig=/tmp/x", "get", "pods"],
            &["--kubeconfig", "/tmp/x", "get", "pods"],
        ],
    );
    refused(
        "kubectl",
        "--enable-alpha-plugins",
        &[&["kustomize", "--enable-alpha-plugins", "dir"]],
    );
    refused(
        "kubectl",
        "--enable-exec",
        &[&["kustomize", "--enable-exec", "dir"]],
    );
    refused(
        "kubectl",
        "--enable-helm",
        &[&["kustomize", "--enable-helm", "dir"]],
    );
    refused(
        "kubectl",
        "--helm-command",
        &[&["kustomize", "--helm-command=x", "dir"]],
    );
    offered(
        "kubectl",
        &[
            "get pods -n web",
            "--context=prod get pods",
            "apply -f deploy.yaml",
        ],
    );
}

#[test]
fn psql_with_a_file_an_output_pipe_or_a_backslash_command_is_offered_no_run_grant() {
    refused(
        "psql",
        "-f",
        &[
            &["-f", "x.sql"],
            &["-fx.sql"],
            &["-h", "db", "-Xf", "x.sql"],
        ],
    );
    refused("psql", "--file", &[&["--file=x.sql"], &["--fi", "x.sql"]]);
    refused("psql", "-o", &[&["-o", "|sh", "-c", "select 1"]]);
    refused("psql", "--output", &[&["--output=|sh", "-c", "select 1"]]);
    refused(
        "psql",
        "-c",
        &[
            &["-c", "\\! sh"],
            &["-Atc", "\\copy t from program 'sh'"],
            &["-c\\! sh"],
        ],
    );
    refused("psql", "--command", &[&["--command=\\! sh"]]);
    offered(
        "psql",
        &["-h db -U app -c select_1", "-At -c select_1", "-l"],
    );
}

#[test]
fn a_program_that_names_no_command_by_its_flags_is_left_as_it_was() {
    offered(
        "curl",
        &["-o out https://example.com", "-F a=b https://example.com"],
    );
    offered("gh", &["api /user -F x=y"]);
}

#[test]
fn after_a_double_dash_the_words_are_the_programs_and_not_flags() {
    offered("ssh", &["host -- ls -F"]);
}

// The review of #5876: a program's other names, and the forms it missed.

#[test]
fn a_program_named_by_a_suffix_a_version_or_its_newer_build_is_read_by_its_own_flags() {
    refused(
        "kubectl.docker",
        "--kubeconfig",
        &[&["--kubeconfig=/tmp/x", "get", "pods"]],
    );
    refused("psql-16", "-f", &[&["-f", "x.sql"]]);
    refused("rsync3", "-e", &[&["-e", "x", "a", "host:b"]]);
    refused(
        "wget2",
        "--use-askpass",
        &[&["--use-askpass=x", "https://example.com"]],
    );
    refused("wget2", "-e", &[&["-e", "x", "https://example.com"]]);
    offered("kubectl.docker", &["get pods -n web"]);
}

#[test]
fn ssh_with_the_xauth_program_it_names_is_offered_no_run_grant() {
    refused(
        "ssh",
        "-o XAuthLocation",
        &[&["-X", "-o", "XAuthLocation=/tmp/x", "host"]],
    );
    refused(
        "scp",
        "-o XAuthLocation",
        &[&["-oXAuthLocation=/tmp/x", "a", "host:b"]],
    );
}

#[test]
fn mysql_with_a_shell_escape_a_pager_or_a_file_of_settings_is_offered_no_run_grant() {
    refused(
        "mysql",
        "-e with a shell command",
        &[
            &["-e", "\\! sh"],
            &["-e", "system sh"],
            &["-e", "SELECT 1; SYSTEM sh"],
            &["-hdb", "-esystem sh"],
        ],
    );
    refused(
        "mariadb",
        "--execute with a shell command",
        &[&["--execute=\\! sh"]],
    );
    refused("mysql", "--pager", &[&["--pager=sh", "-e", "select 1"]]);
    refused("mysql", "--defaults-file", &[&["--defaults-file=/tmp/x"]]);
    refused(
        "mysql",
        "--defaults-extra-file",
        &[&["--defaults-extra-file=/tmp/x"]],
    );
    refused("mysql", "--plugin-dir", &[&["--plugin-dir=/tmp/x"]]);
    offered(
        "mysql",
        &[
            "-h db -u app -e select_1",
            "-h db -P 3306 -u app -D shop -e show_tables",
        ],
    );
}

// The second review of #5876.

#[test]
fn ssh_reads_an_option_name_in_quotes_so_a_quoted_one_is_refused_too() {
    refused(
        "ssh",
        "-o ProxyCommand",
        &[
            &["-o", "\"ProxyCommand\"=x", "host"],
            &["-o", "'proxycommand' x", "host"],
            &["-o", "Proxy\\Command=x", "host"],
        ],
    );
    refused(
        "scp",
        "-o ProxyCommand",
        &[&["-o\"ProxyCommand\"=x", "a", "host:b"]],
    );
}

#[test]
fn scp_dash_capital_o_takes_no_value_so_what_follows_it_is_read() {
    refused(
        "scp",
        "-S",
        &[&["-O", "-S", "x", "a", "host:b"], &["-OSx", "a", "host:b"]],
    );
    refused("sftp", "-S", &[&["-O", "-S", "x", "host"]]);
}

#[test]
fn mysql_with_a_file_of_commands_in_dash_e_is_offered_no_run_grant() {
    refused(
        "mysql",
        "-e with a shell command",
        &[&["-e", "source /tmp/x.sql"], &["-e", "select 1; SOURCE x"]],
    );
}

#[test]
fn wget2_with_a_plugin_it_loads_is_offered_no_run_grant() {
    refused(
        "wget2",
        "--plugin",
        &[&["--plugin=x", "https://example.com"]],
    );
    refused(
        "wget",
        "--local-plugin",
        &[&["--local-plugin=/tmp/x.so", "https://example.com"]],
    );
    refused(
        "wget2",
        "--plugin-dirs",
        &[&["--plugin-dirs=/tmp", "https://example.com"]],
    );
}
