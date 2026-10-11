### Fixed

- **`purlis plugin install` and `purlis doctor` find the plugin a Linux package ships.** They
  look in `/usr/lib/purlis/plugin`, where the package keeps it, and still in
  `/usr/lib/charter/plugin` for a package built before the rename. purlis's requests to a forge
  carry the `purlis/<version>` user agent (#1284).
