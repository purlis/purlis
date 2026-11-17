// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether an engine's host kernel has the Landlock purlis's sandbox needs** (#7084, ADR 0123
//! as amended 2026-11-17). A container runs on its host's kernel, and a runner starts every chat
//! inside purlis's sandbox, which needs Landlock ABI [`purlis_linux_sandbox::MIN_ABI`] (Linux
//! 6.12). So a pool whose host lacks it starts no chat, and Pool setup says so before one is
//! tried.
//!
//! Two readings, the first that answers deciding:
//!
//! - **The kernel's own answer**, given an image: the image's `purlis _landlock`
//!   ([`purlis_linux_sandbox::LANDLOCK`]) prints the ABI the kernel grants, in a container held as
//!   the namespace probe is and by the runner's own seccomp profile, so it answers as a runner's
//!   chat would be answered. An image whose `purlis` predates the command prints no number.
//! - **The release `docker info` names** (`KernelVersion`), where the image didn't answer or none
//!   is picked yet: before 6.12 is short. A kernel can lack Landlock at 6.12 (left out of its
//!   security modules) or, rarely, have it earlier as a backport; the image's answer, when there
//!   is one, says which.
//!
//! Neither read holds nothing up and claims nothing ([`HostKernel::Unread`]). Both are read as
//! data: a number printed, and `docker info`'s fields.

use super::{Docker, Image, MOST_SAID, PROBE_WITHIN, SECCOMP_FILE, SECCOMP_PROFILE};

/// The first Linux release with Landlock ABI [`purlis_linux_sandbox::MIN_ABI`].
pub const FLOOR: (u32, u32) = (6, 12);

/// **What a host's kernel was found to be.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKernel {
    /// It has the Landlock purlis's sandbox needs.
    Enough,
    /// It lacks it: no chat starts on a runner there.
    Short(Short),
    /// Neither the kernel nor `docker info` said.
    Unread,
}

/// **A host kernel that lacks the Landlock purlis's sandbox needs**: its release where `docker
/// info` named one, and whether the host is Ubuntu 24.04, whose newer kernel is one package away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Short {
    pub release: Option<String>,
    pub ubuntu_24_04: bool,
}

/// **What `docker info` says of the host**: `KernelVersion` and `OperatingSystem`, each where it
/// is said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub release: Option<String>,
    pub system: Option<String>,
}

impl Info {
    /// The fields of `docker info --format '{{json .}}'`, or of Podman's, where `info` holds them.
    #[must_use]
    pub fn of(info: &serde_json::Value) -> Info {
        let field = |name: &str, podman: &str| {
            info.get(name)
                .or_else(|| info.pointer(podman))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        Info {
            release: field("KernelVersion", "/host/kernel"),
            system: field("OperatingSystem", "/host/distribution/distribution"),
        }
    }
}

/// **The judgement**: `abi`, the kernel's own answer, where there is one; else the release
/// `info` names, against [`FLOOR`]; else unread.
#[must_use]
pub fn judged(abi: Option<i32>, info: &Info) -> HostKernel {
    let enough = match abi {
        Some(abi) => abi >= purlis_linux_sandbox::MIN_ABI,
        None => match info.release.as_deref().and_then(release_of) {
            Some(release) => release >= FLOOR,
            None => return HostKernel::Unread,
        },
    };
    if enough {
        HostKernel::Enough
    } else {
        HostKernel::Short(Short {
            release: info.release.clone(),
            ubuntu_24_04: info
                .system
                .as_deref()
                .is_some_and(|system| system.starts_with("Ubuntu 24.04")),
        })
    }
}

/// **A kernel release's major and minor** (`6.8.0-45-generic` is 6.8): `None` where it doesn't
/// start with two numbers.
#[must_use]
pub fn release_of(release: &str) -> Option<(u32, u32)> {
    let mut parts = release.trim().split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// **The ABI `purlis _landlock` printed**: one number on its own, or `None`.
#[must_use]
pub fn abi_printed(printed: &str) -> Option<i32> {
    printed.trim().parse().ok()
}

impl Docker {
    /// **Whether the engine's host kernel has the Landlock purlis's sandbox needs** (#7084), by
    /// `image`'s own answer where an image is named, else by `docker info` alone ([`judged`]).
    #[must_use]
    pub fn host_kernel(&self, image: Option<&Image>) -> HostKernel {
        let info = self
            .json(&["info", "--format", "{{json .}}"], "docker info")
            .map(|info| Info::of(&info))
            .unwrap_or_default();
        judged(image.and_then(|image| self.landlock_of(image)), &info)
    }

    /// **The Landlock ABI the kernel grants a container of `image`**, as its `purlis _landlock`
    /// prints it: run as the image's own user, with no network, every capability dropped, no
    /// new privileges, its root read-only and the runner's own seccomp profile, which lets the
    /// Landlock calls through as a runner's does. `None` where it printed no number in
    /// [`PROBE_WITHIN`]; its container is removed either way.
    fn landlock_of(&self, image: &Image) -> Option<i32> {
        let prepared = self.prepared().ok()?;
        let here = prepared.keep(SECCOMP_FILE, SECCOMP_PROFILE).ok()?;
        // Podman's client sends the profile's path, which its engine reads on its own machine,
        // as a runner's is kept there (#6269).
        let there = match &self.reach {
            Some(reach) => Some(reach.keep(SECCOMP_FILE, SECCOMP_PROFILE).ok()?),
            None => None,
        };
        let seccomp = there.as_deref().unwrap_or(&here);
        let folder = tempfile::tempdir().ok()?;
        let cidfile = folder.path().join("landlock.cid");
        let args: Vec<String> = [
            "run",
            "--rm",
            "--cidfile",
            &cidfile.display().to_string(),
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--security-opt",
            &format!("seccomp={}", seccomp.display()),
            "--network",
            "none",
            "--read-only",
            "--entrypoint",
            super::PURLIS,
            image.as_str(),
            purlis_linux_sandbox::LANDLOCK,
        ]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
        let ran = self.spawned(&prepared, &args, None, MOST_SAID, Some(PROBE_WITHIN));
        let abi = match &ran {
            Ok(ran) if ran.status.success() => abi_printed(&String::from_utf8_lossy(&ran.printed)),
            _ => None,
        };
        if ran.is_err() {
            // One that didn't end in its time may be left: the engine's id for it removes it.
            if let Ok(id) = std::fs::read_to_string(&cidfile)
                && let Ok(id) = super::ContainerId::new(id.trim())
            {
                let rm = [
                    "rm".to_owned(),
                    "--force".to_owned(),
                    id.as_str().to_owned(),
                ];
                let _ = self.run(&rm, None, MOST_SAID);
            }
        }
        abi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(release: Option<&str>, system: Option<&str>) -> Info {
        Info {
            release: release.map(str::to_owned),
            system: system.map(str::to_owned),
        }
    }

    #[test]
    fn a_release_is_its_major_and_minor_whatever_follows() {
        assert_eq!(release_of("6.8.0-45-generic"), Some((6, 8)));
        assert_eq!(release_of("6.12.48+deb13-amd64"), Some((6, 12)));
        assert_eq!(release_of("6.10.14-linuxkit"), Some((6, 10)));
        assert_eq!(release_of("5.14.0-503.11.1.el9_5.x86_64"), Some((5, 14)));
        assert_eq!(release_of("linuxkit"), None);
        assert_eq!(release_of("6"), None);
    }

    #[test]
    fn the_kernels_own_answer_decides_where_there_is_one() {
        let old = info(Some("6.8.0-45-generic"), Some("Ubuntu 24.04.1 LTS"));
        assert_eq!(judged(Some(6), &old), HostKernel::Enough);
        let new = info(Some("6.14.0-29-generic"), None);
        assert_eq!(
            judged(Some(0), &new),
            HostKernel::Short(Short {
                release: Some("6.14.0-29-generic".into()),
                ubuntu_24_04: false,
            })
        );
    }

    #[test]
    fn without_the_kernels_answer_the_release_decides_and_without_either_nothing_does() {
        assert_eq!(
            judged(
                None,
                &info(Some("6.8.0-45-generic"), Some("Ubuntu 24.04.1 LTS"))
            ),
            HostKernel::Short(Short {
                release: Some("6.8.0-45-generic".into()),
                ubuntu_24_04: true,
            })
        );
        assert_eq!(
            judged(None, &info(Some("6.12.0"), None)),
            HostKernel::Enough
        );
        assert_eq!(judged(None, &info(Some("6.11.9"), None)).clone(), {
            HostKernel::Short(Short {
                release: Some("6.11.9".into()),
                ubuntu_24_04: false,
            })
        });
        assert_eq!(judged(None, &info(None, None)), HostKernel::Unread);
        assert_eq!(
            judged(None, &info(Some("linuxkit"), None)),
            HostKernel::Unread
        );
    }

    #[test]
    fn only_a_number_on_its_own_is_an_abi() {
        assert_eq!(abi_printed("6\n"), Some(6));
        assert_eq!(abi_printed("  4 "), Some(4));
        assert_eq!(abi_printed(""), None);
        assert_eq!(abi_printed("purlis: unknown command _landlock\n"), None);
    }

    #[test]
    fn docker_and_podman_info_name_the_release_and_system() {
        let docker = serde_json::json!({
            "KernelVersion": "6.8.0-45-generic",
            "OperatingSystem": "Ubuntu 24.04.1 LTS"
        });
        assert_eq!(
            Info::of(&docker),
            info(Some("6.8.0-45-generic"), Some("Ubuntu 24.04.1 LTS"))
        );
        let podman = serde_json::json!({
            "host": {"kernel": "6.1.0-26-amd64", "distribution": {"distribution": "debian"}}
        });
        assert_eq!(
            Info::of(&podman),
            info(Some("6.1.0-26-amd64"), Some("debian"))
        );
        assert_eq!(
            Info::of(&serde_json::json!({"KernelVersion": " "})),
            Info::default()
        );
    }
}
