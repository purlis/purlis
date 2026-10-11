//! The files a workspace keeps beside its four stores — `workspace.md`, `workspace.json`, the
//! piece claim log in `pieces/` and the work link log in `work/` — used through the workspace's
//! folder held by descriptor (#1082, V74), as its stores are.
//!
//! Their writers name each file by its path and answer in the words a by-path write always gave
//! ([`crate::rewrite`]): the same path gates are asked first, so a link out of the project is
//! refused as it always was, a read-only file is "`<path>` is read-only", and the filesystem's
//! own refusal names the file ([`crate::rewrite::refused_at`]). What the hold adds is that a
//! link on the way, even one that stays inside the project, is never followed: the write is
//! refused and nothing is written, however late the link was put there.

use std::io;
use std::path::Path;

use super::{Failed, Make, Store, Unheld, Who};

/// The folders `below` workspace `ws`'s own folder (none for the folder itself), held. Made on
/// the way when `make` is true, as `create_dir_all` made them; `Ok(None)` when one is not there
/// and is not to be made.
pub(crate) fn folder(
    root: &Path,
    ws: &str,
    below: &[&str],
    make: bool,
) -> io::Result<Option<Store>> {
    let wanted = if make { Make::All } else { Make::Nothing };
    let Some(mut held) = Store::workspace(root, ws, wanted, Who::Operator).map_err(unheld)? else {
        return Ok(None);
    };
    for dir in below {
        held = match held.sub(dir, make).map_err(unheld)? {
            Some(below) => below,
            None => return Ok(None),
        };
    }
    Ok(Some(held))
}

/// The folder `path` is in, `below` workspace `ws`'s own folder, held, and made on the way when
/// it is not there, as `create_dir_all` made it: a folder the filesystem refuses to make is
/// named by `path`'s folder in the words a by-path write gave ([`crate::rewrite::refused_at`],
/// #1359). Each folder is made in the one held above it, never through a link.
pub(crate) fn made(root: &Path, ws: &str, below: &[&str], path: &Path) -> io::Result<Store> {
    let dir = path.parent().unwrap_or(path);
    let refused = |e: rustix::io::Errno| crate::rewrite::refused_at(dir)(e.into());
    let mut held = match Store::workspace(root, ws, Make::Nothing, Who::Operator).map_err(unheld)? {
        Some(held) => held,
        None => {
            let project = Store::project(root).map_err(unheld)?;
            project.make_dir("workspaces").map_err(refused)?;
            let workspaces = project
                .sub("workspaces", false)
                .map_err(unheld)?
                .ok_or_else(|| refused(rustix::io::Errno::NOENT))?;
            workspaces.make_dir(ws).map_err(refused)?;
            Store::workspace(root, ws, Make::Nothing, Who::Operator)
                .map_err(unheld)?
                .ok_or_else(|| refused(rustix::io::Errno::NOENT))?
        }
    };
    for name in below {
        held.make_dir(name).map_err(refused)?;
        held = held
            .sub(name, false)
            .map_err(unheld)?
            .ok_or_else(|| refused(rustix::io::Errno::NOENT))?;
    }
    Ok(held)
}

/// A folder that could not be held, as an `io::Error`: the filesystem's own error when it
/// refused a look, a refusal otherwise.
pub(crate) fn unheld(unheld: Unheld) -> io::Error {
    match unheld.errno {
        Some(errno) => io::Error::from_raw_os_error(errno),
        None => io::Error::new(io::ErrorKind::PermissionDenied, unheld.why),
    }
}

/// A write to `path` that did not happen, in the words a by-path write of it gave.
pub(crate) fn written(path: &Path) -> impl FnOnce(Failed) -> io::Error + '_ {
    move |failed| match failed {
        Failed::Refused(why) => io::Error::new(io::ErrorKind::PermissionDenied, why),
        Failed::ReadOnly => io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is read-only", path.display()),
        ),
        Failed::Io(e) => crate::rewrite::refused_at(path)(e),
    }
}

/// `name`'s text in `held`, exactly as it is on disk and whatever its size, as
/// `std::fs::read_to_string` read it by path: `None` when it is not there. A link, a file with
/// another name or text that is not UTF-8 is an error, never read through.
pub(crate) fn read(held: &Store, name: &str) -> io::Result<Option<String>> {
    held.read_log(name).map_err(io::Error::other)
}

/// Append `line` to `name` in `held`, made `0644` (before the umask) when it is not there, as
/// the by-path logs were opened (`O_APPEND`, never through a link).
pub(crate) fn append(held: &Store, name: &str, line: &[u8], path: &Path) -> io::Result<()> {
    let mode = rustix::fs::Mode::from_raw_mode(0o644);
    held.append_at(name, "", line, mode).map_err(written(path))
}

/// Replace `name` in `held` whole with `bytes`, keeping its mode, as [`crate::rewrite::replace`]
/// replaced it by path with [`crate::rewrite::Mode::Kept`].
pub(crate) fn replace(held: &Store, name: &str, bytes: &[u8], path: &Path) -> io::Result<()> {
    held.replace_at(name, bytes).map_err(written(path))
}

/// Where `path` is in workspace `ws` of the project at `root`: the folders below the workspace's
/// own, one plain name each, and the file's name. `None` for a path that is not below
/// `<root>/workspaces/<ws>/` or that holds anything but plain names (a `..`, a `.`), so a hold
/// never rests on how a caller built the path.
pub(crate) fn split(root: &Path, ws: &str, path: &Path) -> Option<(Vec<String>, String)> {
    let below = path.strip_prefix(root.join("workspaces").join(ws)).ok()?;
    let mut parts = below
        .components()
        .map(|part| match part {
            std::path::Component::Normal(name) => name.to_str().map(str::to_owned),
            _ => None,
        })
        .collect::<Option<Vec<String>>>()?;
    let name = parts.pop()?;
    Some((parts, name))
}

/// The folder `path` is in, below workspace `ws`, held and made when it is not there ([`made`]),
/// with the file's name: `path` read by [`split`], and refused when it is not one it reads.
pub(crate) fn made_for(root: &Path, ws: &str, path: &Path) -> io::Result<(Store, String)> {
    let (dirs, name) = split(root, ws, path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a file of workspace '{ws}'", path.display()),
        )
    })?;
    let dirs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    Ok((made(root, ws, &dirs, path)?, name))
}
