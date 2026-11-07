### Security

- **A 2FA seed in any mix of upper and lower case is not filled into a page.** A value that
  looks like a seed counts as one whatever its case, so a seed with its first letter
  capitalised, or in random case, needs the no-seed mark, as one in a single case already did.
  A seed joined to a name before it by `_` or `-` (`secret_GEZD…`) counts too. A password of 16
  or more characters drawn only from base32's alphabet (the letters, and the digits 2 to 7) now
  needs that mark as well: one with a 0, 1, 8 or 9, or any other mark, still fills.
