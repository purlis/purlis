### Changed

- The Organisation's policy folder (`/etc/purlis`, or `%ProgramData%\purlis` on Windows) is now checked for its owner and access-control list on every OS before purlis trusts it, including a macOS ACL that lets someone else write. A folder that fails reads as strict with every lock, and the Notice names the failed check and how an administrator can fix it or remove the folder. `policy.json` is translated into the Organisation's rules and locks, and a process outside every project is held to them too.
