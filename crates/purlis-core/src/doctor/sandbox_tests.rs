//! The `sandbox local ports` row (#1699): an administrator's Claude Code setting that turns
//! local binding on is said, with the file that sets it; nothing is said where none does.

use std::path::PathBuf;

use super::local_ports_row;
use crate::doctor::Status;
use crate::sandbox::claude::LocalBinding;

#[test]
fn an_administrators_local_binding_is_said_with_its_file() {
    let file = PathBuf::from("/managed/managed-settings.json");
    let row = local_ports_row(&LocalBinding::On(file)).expect("a row");
    assert_eq!(row.name, "sandbox local ports");
    assert_eq!(
        row.status,
        Status::Ok,
        "an administrator's choice, not a fault"
    );
    assert!(
        row.detail.contains("/managed/managed-settings.json")
            && row.detail.contains("every port on this machine"),
        "{}",
        row.detail
    );
}

#[test]
fn a_managed_file_purlis_cannot_read_is_a_warning() {
    let row = local_ports_row(&LocalBinding::Unread {
        file: PathBuf::from("/managed/10-x.json"),
        why: "permission denied".to_owned(),
    })
    .expect("a row");
    assert_eq!(row.status, Status::Warn);
    assert!(row.detail.contains("/managed/10-x.json"), "{}", row.detail);
}

#[test]
fn nothing_is_said_where_no_administrator_turns_it_on() {
    assert_eq!(local_ports_row(&LocalBinding::Off), None);
}
