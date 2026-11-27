### Fixed

- **A forge refusal says what was blocked and why.** A chat refused by the forge route used to be
  told `NotOnList`, which named neither the act nor the repo. Now it is told one of two things: that
  purlis doesn't carry the request, shown as its route's pattern on the repo, and that no setting
  can allow it (for the repo's administration, a rule over the repo or an approval, also that it's
  yours to do on the forge); or that a known act, a merge, a release or tag or a fork, isn't allowed
  on that repo, why, and that nobody was asked, with the exact form purlis can hold for your press.
  A merge refused only because this machine hasn't accepted the project's policy says so. A
  request that only looks like such an act, such as a label named `merge`, is told that purlis
  doesn't carry it, and a caller that didn't prove it's the chat is told nothing of where its
  request went. Nothing the chat chose is shown: numbers read `{n}`, other words `{…}`, and the
  query is left out. Each such refusal counts under Settings › Project › Policy, Would have
  blocked (#7779).
