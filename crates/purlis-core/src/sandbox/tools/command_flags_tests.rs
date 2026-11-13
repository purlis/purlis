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

// The third review of #5876.

#[test]
fn rsync_hands_an_operands_host_to_ssh_so_one_after_a_double_dash_starting_with_a_dash_is_refused()
{
    refused(
        "rsync",
        "an operand starting with -",
        &[
            &["-a", "--", "-oProxyCommand=x:path", "dst"],
            &["--", "a", "-ox:b"],
        ],
    );
    offered("rsync", &["-a -- src host:dst"]);
}

#[test]
fn ssh_with_a_jump_host_or_name_a_shell_would_read_is_offered_no_run_grant() {
    refused(
        "ssh",
        "-J",
        &[&["-J", "bastion;x", "host"], &["-Jx$(y)", "host"]],
    );
    refused("ssh", "-o ProxyJump", &[&["-o", "ProxyJump=a|b", "host"]]);
    refused("ssh", "-o Hostname", &[&["-o", "Hostname=a`b`", "host"]]);
    refused("ssh", "-o User", &[&["-o", "User=a;b", "host"]]);
    offered(
        "ssh",
        &[
            "-J deploy@bastion.example.com:2222 host",
            "-J a,b host",
            "-o ProxyJump=bastion host",
            "-o Hostname=10.0.0.5 host",
            "-o User=deploy host",
            "-o UserKnownHostsFile=/dev/null host",
            "-o HostKeyAlias=web host",
        ],
    );
}

#[test]
fn ssh_keygen_and_ssh_add_are_read_by_their_own_flags_not_sshs() {
    offered("ssh-keygen", &["-F host", "-t ed25519 -f key -N x"]);
    refused("ssh-keygen", "-D", &[&["-D", "/tmp/x.so"]]);
    refused(
        "ssh-keygen",
        "-w",
        &[&["-t", "ed25519-sk", "-w", "/tmp/x.so"]],
    );
    refused("ssh-add", "-s", &[&["-s", "/tmp/x.so"]]);
    refused("ssh-add", "-S", &[&["-S", "/tmp/x.so", "key"]]);
    offered("ssh-add", &["key", "-l"]);
}

#[test]
fn mysql_with_its_editor_or_pager_command_in_dash_e_is_offered_no_run_grant() {
    refused(
        "mysql",
        "-e with a shell command",
        &[&["-e", "edit"], &["-e", "pager sh"]],
    );
}

// The fourth review of #5876.

#[test]
fn rsync_hands_a_user_and_host_to_ssh_as_words_of_their_own_so_one_starting_with_a_dash_is_refused()
{
    refused(
        "rsync",
        "an operand starting with -",
        &[
            &["-a", "u@-ox:path", "dst"],
            &["-a", "--", "u@-ox:path", "dst"],
            &["src", "u@-ox::module"],
        ],
    );
    offered(
        "rsync",
        &[
            "-a src deploy@host:dst",
            "-a src host::module",
            "-a -- src host:dst",
        ],
    );
}

#[test]
fn ssh_with_a_jump_or_host_starting_with_a_dash_is_offered_no_run_grant() {
    refused(
        "ssh",
        "-J",
        &[
            &["-J", "-ox", "host"],
            &["-J", "u@-ox", "host"],
            &["-J", "a,-ox", "host"],
        ],
    );
    refused("ssh", "-o ProxyJump", &[&["-o", "ProxyJump=-ox", "host"]]);
    refused("ssh", "-o Hostname", &[&["-o", "Hostname=-ox", "host"]]);
}

#[test]
fn mysqls_own_tools_are_not_read_by_the_clients_flags() {
    refused(
        "mariadb-dump",
        "--plugin-dir",
        &[&["-e", "--plugin-dir=/tmp/x", "shop"]],
    );
    refused(
        "mysqldump",
        "--defaults-file",
        &[&["-e", "--defaults_file=/tmp/x", "shop"]],
    );
    offered("mariadb-dump", &["-e --single-transaction shop"]);
    offered("mysqladmin", &["-h db status"]);
    refused("mysql-8.0", "--pager", &[&["--pager=x"]]);
}

// The fifth review of #5876.

#[test]
fn a_host_in_brackets_starting_with_a_dash_is_refused_as_one_without() {
    refused(
        "ssh",
        "-J",
        &[
            &["-J", "[-ox]", "host"],
            &["-J", "u@[-ox]:22", "host"],
            &["-J", "ssh://[-ox]", "host"],
        ],
    );
    refused(
        "rsync",
        "an operand starting with -",
        &[
            &["src", "[-ox]:dst"],
            &["src", "u@[-ox]:dst"],
            &["--", "src", "[-ox]:dst"],
        ],
    );
    offered("ssh", &["-J [::1]:2222 host", "-J u@[fe80::1] host"]);
    offered("rsync", &["src [::1]:dst", "src u@[fe80::1]:dst"]);
}

#[test]
fn every_tool_named_for_mysql_or_mariadb_but_the_client_is_read_by_the_tools_row() {
    refused(
        "mariadb-binlog",
        "--defaults-file",
        &[&["-D", "--defaults-file=/tmp/x", "log"]],
    );
    refused(
        "mysqlbinlog",
        "--plugin-dir",
        &[&["--plugin-dir=/tmp/x", "log"]],
    );
    refused(
        "mysqlcheck",
        "--defaults-file",
        &[&["--defaults-file=/tmp/x"]],
    );
    refused("mysql_install_db", "--basedir", &[&["--basedir=/tmp/x"]]);
    refused("mysql-8.0", "--pager", &[&["--pager=x"]]);
    offered("mariadb-binlog", &["-D log"]);
}

#[test]
fn ssh_with_a_jump_and_a_destination_a_shell_would_read_is_offered_no_run_grant() {
    refused(
        "ssh",
        "a destination a shell would read",
        &[
            &["-J", "bastion", "h;x"],
            &["-o", "ProxyJump=bastion", "u@h$(x)", "ls"],
            &["-J", "bastion", "--", "h|x"],
        ],
    );
    offered(
        "ssh",
        &[
            "-J bastion deploy@host ls -la",
            "host echo a;b",
            "-J bastion -- host uptime",
        ],
    );
}

// The sixth review of #5876.

#[test]
fn scp_and_sftp_with_a_jump_and_a_host_a_shell_would_read_are_offered_no_run_grant() {
    refused(
        "scp",
        "a destination a shell would read",
        &[&["-J", "b", "h$(x):f", "."], &["-J", "b", ".", "u@[h;x]:f"]],
    );
    refused(
        "sftp",
        "a destination a shell would read",
        &[&["-J", "b", "h|x"], &["-o", "ProxyJump=b", "u@h;x:dir"]],
    );
    offered(
        "scp",
        &["-J b ~/a.txt deploy@host:~/b.txt", "-J b host:a ./my;file"],
    );
    offered("sftp", &["-J b deploy@host", "-J b host:dir"]);
}

// The seventh review of #5876.

#[test]
fn scp_and_sftp_uris_hosts_are_held_to_a_jumps_rule_and_a_slash_before_a_colon_is_a_local_path() {
    refused(
        "scp",
        "a destination a shell would read",
        &[
            &["-J", "b", "scp://h$(x)/f", "."],
            &["-J", "b", "a", "scp://u@[h;x]:22/f"],
        ],
    );
    refused(
        "sftp",
        "a destination a shell would read",
        &[&["-J", "b", "sftp://h|x"]],
    );
    offered(
        "scp",
        &[
            "-J b scp://deploy@host:2222/f .",
            "-J b ./dir;x/y:z host:dst",
        ],
    );
    offered("sftp", &["-J b sftp://deploy@[::1]:22/dir"]);
}

// The eighth review of #5876: a `[` that does not open the host is an ordinary character to scp
// and rsync, so the `:` after it still ends the host.

#[test]
fn a_bracket_inside_a_host_does_not_hide_the_colon_that_ends_it() {
    refused(
        "scp",
        "a destination a shell would read",
        &[&["-J", "b", "f", "h$(x)[:p"], &["-J", "b", "f", "a[b:c]"]],
    );
    refused(
        "rsync",
        "an operand starting with -",
        &[&["src", "u@-oX[:dst"], &["--", "src", "u@-oX[:dst"]],
    );
    offered("scp", &["-J b f [::1]:p", "-J b f u@[fe80::1]:p"]);
}

// #5892, from the ninth review of #5876: ssh pastes a jump list unquoted into the command its
// shell runs, brackets and all, so a bracketed host holds only an IPv6 address's characters and
// is never a glob of anything else.

#[test]
fn a_bracketed_jump_host_holds_only_an_ipv6_addresss_characters() {
    refused(
        "ssh",
        "-J",
        &[
            &["-J", "a,[.-_]", "host"],
            &["-J", "[g::1]", "host"],
            &["-J", "u@[fe80::1_x]:22", "host"],
        ],
    );
    refused("ssh", "-o ProxyJump", &[&["-o", "ProxyJump=[a-b]", "host"]]);
    refused(
        "scp",
        "a destination a shell would read",
        &[&["-J", "b", "f", "[a_b]:p"]],
    );
    offered(
        "ssh",
        &[
            "-J [::ffff:10.0.0.1]:22 host",
            "-J a,u@[2001:DB8::1] host",
            "-o ProxyJump=[::1] host",
        ],
    );
}

// #5893, from the ninth review of #5876: ssh expands its `%` tokens (`%h`, `%r`, `%n`, `%p`) in
// a jump's whole command, and only `%%` stands for a `%` itself. A scoped link-local jump host is
// offered only spelled so, inside its brackets, as the last hop (the hops before it are pasted
// into the next ssh's jump and expanded again), and nowhere else.

#[test]
fn a_scoped_link_local_jump_host_is_offered_only_with_its_zone_spelled_as_ssh_reads_it() {
    offered(
        "ssh",
        &[
            "-J u@[fe80::1%%en0] host",
            "-J a,[fe80::1%%eth0.100]:2222 host",
            "-J [FEBF::1%%br-lan_2] host",
            "-o ProxyJump=b,deploy@[fe80::1%%en0] host",
        ],
    );
    refused(
        "ssh",
        "-J",
        &[
            // a single `%` forms a token: `%r` is the user `-l` names
            &["-J", "[fe80::1%r]", "-l", "$(x)", "host"],
            &["-J", "[fe80::1%en0]", "host"],
            // expanded again in the next ssh's jump
            &["-J", "[fe80::1%%en0],a", "host"],
            &["-J", "[fe80::1%%lo0],[fe80::2%%en0]", "host"],
            // not link-local
            &["-J", "[2001:db8::1%%en0]", "host"],
            &["-J", "[fec0::1%%en0]", "host"],
            &["-J", "[fe8::1%%en0]", "host"],
            // outside brackets, empty, twice, or holding what else
            &["-J", "fe80::1%%en0", "host"],
            &["-J", "[fe80::1%%]", "host"],
            &["-J", "[%%en0]", "host"],
            &["-J", "[fe80::1%%en0%%x]", "host"],
            &["-J", "[fe80::1%%e%n]", "host"],
            &["-J", "[fe80::1%%e$n]", "host"],
            &["-J", "[fe80::1%%e[n]", "host"],
            &["-J", "u%%x@[fe80::1%%en0]", "host"],
            &["-J", "[fe80::1%%en0]:2%%2", "host"],
            &["-J", "ssh://[fe80::1%%en0]", "host"],
            &["-J", "SSH://[fe80::1%%en0]", "host"],
        ],
    );
    refused(
        "ssh",
        "-o Hostname",
        &[&["-o", "Hostname=[fe80::1%%en0]", "host"]],
    );
    refused(
        "ssh",
        "a destination a shell would read",
        &[&["-J", "b", "[fe80::1%%en0]"]],
    );
}

// The second review of #5929: brackets in a jump's user are a glob too, since ssh pastes the
// user as `-l user` unquoted; and a zone is let through in no `ssh://` hop, with a user or not.

#[test]
fn a_jump_users_brackets_and_a_zone_in_an_ssh_uri_with_a_user_are_offered_no_run_grant() {
    refused(
        "ssh",
        "-J",
        &[
            &["-J", "[x]@host", "host"],
            &["-J", "a]@host", "host"],
            &["-J", "[x]@[fe80::1%%en0]", "host"],
            &["-J", "ssh://u@[fe80::1%%en0]", "host"],
            &["-J", "a,ssh://u@[fe80::1%%en0]", "host"],
        ],
    );
    refused("ssh", "-o ProxyJump", &[&["-o", "ProxyJump=[x]@b", "host"]]);
    offered("ssh", &["-J u@[::1] host", "-J ssh://u@[::1]:22 host"]);
}

/// RocksDB's `ldb` and `sst_dump`, as MariaDB's RocksDB plugin ships them (#6322): their
/// `--env_uri` and `--fs_uri` name an object RocksDB's registry makes, which loads no library
/// (`tools/ldb_cmd.cc`, `env/env.cc`, `utilities/object_registry.cc`, RocksDB 79f08d7, as
/// MariaDB 11.4 pins it), so their usual commands stay offered, by the prefix that takes
/// `mariadb-ldb` and `mysql_ldb` to the mysql tools' row too.
#[test]
fn rocksdbs_ldb_and_sst_dump_are_offered_with_their_usual_commands() {
    for program in ["mariadb-ldb", "mysql_ldb", "ldb", "sst_dump"] {
        offered(
            program,
            &[
                "--db=/var/lib/mysql/#rocksdb scan",
                "--db=/var/lib/mysql/#rocksdb --env_uri=x get key",
                "--file=/var/lib/mysql/#rocksdb/000001.sst --command=raw",
            ],
        );
    }
}

/// openssl's options that load a library or a module (#6329), read as its `opt_next` reads them
/// (`apps/lib/opt.c`, openssl 3.0.15): one or two dashes, the whole name, a value after `=` or
/// in the next word. `-engine` and its kin load the shared library they are named through the
/// dynamic engine's `SO_PATH` (`apps/lib/engine.c`), `-provider` and `-provider-path` a provider
/// module (`apps/lib/app_provider.c`), and `-config` a file whose modules `ca`, `req`, `ts`,
/// `srp`, `cms`, `smime`, `genpkey` and `pkeyutl` load (`app_load_modules`, `apps/lib/apps.c`).
/// `my_openssl` is the bundled openssl MySQL's test package ships.
#[test]
fn openssl_with_an_option_that_loads_a_library_is_offered_no_run_grant() {
    for program in ["openssl", "my_openssl", "openssl3"] {
        refused(
            program,
            "-engine",
            &[
                &["dgst", "-engine", "/x/evil.so", "-sha256", "f"],
                &["req", "--engine=/x/evil.so", "-new"],
                &["rsa", "-ENGINE", "/x/evil.so"],
            ],
        );
        refused(
            program,
            "-keygen_engine",
            &[&["req", "-keygen_engine", "x"]],
        );
        refused(
            program,
            "-ssl_client_engine",
            &[&["s_client", "-ssl_client_engine", "x", "-connect", "h:443"]],
        );
        refused(
            program,
            "-provider",
            &[
                &["list", "-provider", "/x/evil.so"],
                &["dgst", "-provider=evil", "f"],
            ],
        );
        refused(
            program,
            "-provider-path",
            &[&["list", "-provider-path", "/x", "-providers"]],
        );
        refused(
            program,
            "-config",
            &[
                &["req", "-config", "/x/openssl.cnf", "-new"],
                &["ca", "--config=/x/c"],
            ],
        );
    }
}

/// openssl's `engine` command loads the engines it is named, and runs the control commands its
/// `-pre` and `-post` name (`SO_PATH:<library>`, `LOAD`), and `fipsinstall` loads the provider
/// its `-module` folder holds (`apps/engine.c`, `apps/fipsinstall.c`, openssl 3.0.15).
#[test]
fn openssl_engine_and_fipsinstall_are_offered_no_run_grant() {
    for program in ["openssl", "my_openssl"] {
        refused(
            program,
            "engine",
            &[&["engine", "dynamic", "-pre", "SO_PATH:/x.so"]],
        );
        refused(program, "fipsinstall", &[&["fipsinstall", "-out", "f.cnf"]]);
    }
}

/// openssl's usual commands stay offered (#6329): the general-purpose caution says what it may
/// do with what it is handed, and a short option is never read as the start of a long one.
#[test]
fn openssl_is_offered_with_its_usual_commands() {
    for program in ["openssl", "my_openssl"] {
        offered(
            program,
            &[
                "dgst -sha256 f",
                "s_client -connect host:443 -servername host",
                "enc -e -aes-256-cbc -pbkdf2 -in a -out b -pass env:X",
                "x509 -in c.pem -noout -text",
                "req -new -key k.pem -subj /CN=x -out r.csr",
                "pkcs12 -export -in c.pem -inkey k.pem -out p.p12",
                "rand -hex 16",
                "version",
            ],
        );
    }
}
