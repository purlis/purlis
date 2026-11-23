### Fixed

- **A login's hold on the other pages now waits until each is really frozen.** A page of the
  login's site in another tab could still run one last timer after purlis had cleared the site,
  and put an earlier session's cookie back. purlis now confirms each page is frozen before it
  clears anything, and fills nothing if one can't be confirmed.
  A cookie a page had already sent just before it was frozen can still reach the browser a moment
  later, so purlis clears the site again until none is left, and fills nothing if one keeps coming
  back.
