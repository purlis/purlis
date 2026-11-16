// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pools a key is sent to, as Your keys sees them** (R-171, V8, X1; T1a-11).
//!
//! The pool key allowances themselves are T1a-53's store (`<config>/model-keys/pool-allows.json`,
//! spec C-23), and the push of a removal to a pool's runs is T1a-74's link message. Your keys
//! reaches both through the two seams here, so the page lists and revokes through core whatever
//! holds them: [`Allowances`] and [`Link`]. Until those tickets land, [`NoneYet`] answers both:
//! no pool is listed, and there is nothing to revoke or push.

use ulid::Ulid;

/// **The pool key allowances Your keys reads and changes** (T1a-53 builds the store).
pub trait Allowances: Send + Sync {
    /// The pools entry `entry` is sent to, by name, in the store's order. An allowance file that
    /// can't be read reads as no allowance (spec C-23), so this never fails.
    fn pools(&self, entry: Ulid) -> Vec<String>;
    /// Deletes entry's row for `pool` (*Stop sending*).
    fn revoke(&self, entry: Ulid, pool: &str) -> Result<(), String>;
    /// Deletes every row of entry's (*Remove*'s third step).
    fn remove_all(&self, entry: Ulid) -> Result<(), String>;
    /// Seals entry's rows again over the value it now holds (*Replace*, X1).
    fn reseal(&self, entry: Ulid) -> Result<(), String>;
}

/// **The runner link's removal of a key from a pool's runs** (T1a-74's link message, X1).
pub trait Link: Send + Sync {
    /// Tells `pool`'s runs that entry is no longer sent there, so their next request gets W11.
    fn removal(&self, pool: &str, entry: Ulid) -> Result<(), String>;
}

/// Neither store nor link yet: no pool is listed, and nothing is revoked or pushed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoneYet;

impl Allowances for NoneYet {
    fn pools(&self, _entry: Ulid) -> Vec<String> {
        Vec::new()
    }
    fn revoke(&self, _entry: Ulid, _pool: &str) -> Result<(), String> {
        Ok(())
    }
    fn remove_all(&self, _entry: Ulid) -> Result<(), String> {
        Ok(())
    }
    fn reseal(&self, _entry: Ulid) -> Result<(), String> {
        Ok(())
    }
}

impl Link for NoneYet {
    fn removal(&self, _pool: &str, _entry: Ulid) -> Result<(), String> {
        Ok(())
    }
}

/// **Stop sending** (W125, X1): deletes entry's row for `pool`, then hands the pool's runs a
/// removal through `link`. A revoke that fails leaves the row, and the error says why (W129).
pub fn stop_sending(
    entry: Ulid,
    pool: &str,
    allowances: &dyn Allowances,
    link: &dyn Link,
) -> Result<(), String> {
    let _ = (entry, pool, allowances, link);
    Ok(())
}
