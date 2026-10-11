### Security

- **A chat's commit and push are refused for an IBAN, eight more national ID numbers and 54
  more vendor tokens.** The scan before a chat commits or pushes now finds an IBAN (its country,
  length and mod-97 check), a UK National Insurance number, a Canadian SIN, an Aadhaar number, a
  German tax ID, a Spanish DNI or NIE, an Italian codice fiscale, a Brazilian CPF and a Dutch
  BSN, each only when its check holds, so a random number is not a hit. An ID written as bare
  digits is found when its name stands in front of it (`BSN: …`) or in its national grouping.
  It also finds every gitleaks default rule held by a fixed vendor marker that it did not find
  already, among them Stripe restricted keys, Vault, Sentry, PlanetScale, Fly.io, Twilio and
  Azure AD client secrets. A refusal names the kind and shows the value masked, as before, and
  personal data is let through by path only, never by fingerprint (#773).
