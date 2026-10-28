### Added

- **A pool can declare its price per hour, and Devices shows its money this week.** A Docker
  pool's row in `runners.json` takes `"pricePerHour": {"amount": "0.40", "currency": "EUR"}`.
  The Devices use table then has a Money this week column: the pool's hours this week times its
  price, worked exactly from the decimal it declares and shown in its currency, never converted.
  A pool that declares no price shows no money (#3491).
