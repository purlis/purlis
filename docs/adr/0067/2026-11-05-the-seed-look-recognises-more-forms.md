# Amended (2026-11-05, spec #5100; the person's ruling of that day): the check for 2FA seeds recognises more of the forms they come in

A plain fill refuses a value that looks like a 2FA seed (#5118). That check now recognises more of
the forms seeds come in, and looks through the whole value for them. It stays a look: a real value
it takes for a seed fills once the person marks its key as no 2FA seed, as it did, and an ordinary
password or a hex key fills with no mark.
