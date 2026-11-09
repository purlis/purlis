### Security

- **A run's notice offers no Allow for an ssh jump whose brackets hold anything but an IPv6
  address** (`-J a,[.-_]`), or whose user holds a bracket (`-J [x]@host`), since ssh pastes a
  jump list unquoted into the command its shell runs (#5892).
