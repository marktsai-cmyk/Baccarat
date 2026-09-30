# To do

## Done

- ~~Configurable rebate~~: see [done.md](done.md#configurable-rebate-srcbetsrs).

## 1. REST API

Serve the calculator and simulator over HTTP so other apps can use them.

- Endpoints (JSON):
  - `POST /analyze`: body `{ "decks": 8, "dealt": ["K", "5"], "payouts": {...} }`, returns each bet's win, lose, push, EV, rebate and win rate excluding ties.
  - `POST /simulate`: body `{ "decks": 8, "cut": 14, "seed": 2026, "payouts": {...} }`, returns every hand and a summary.
  - `GET /health`
- `payouts` includes the rebate: `{ "rebate": { "rates": { "banker": 0.008 }, "on": "decided" } }`.
- Keep the library as it is and add the server as a separate binary or crate, so the command-line tool keeps working.
- Choose a web framework (for example axum on tokio). This adds the project's first outside dependencies.
- Reject bad input with clear 400 errors: too few or too many cards, unknown card, cut card out of range, rebate rate out of range.
- An analysis takes about 1 ms. Cache results for repeated shoes, and put a limit on request sizes.
- Add an OpenAPI description and integration tests for each endpoint.

## Other ideas

- **Rebate on turnover caps:** some casinos pay the rebate only on turnover above a minimum, or cap it per session. The simulator could model this.
- **Many shoes:** a `--shoes N` option that simulates many shoes in parallel and totals the results, to compare with the exact EV.
- **EV during a shoe:** show each bet's exact EV before every simulated hand, to see how the remaining cards change it.
- **More side bets:** Player/Banker Pair, Super 6 / no-commission Banker, Dragon Bonus.
- **Faster analysis:** the side-bet tracking made analysis about 1.5× slower (0.6 ms → 1.1 ms). Grouping deals that give the same hand totals could win this back.
- **Suits in `--dealt`:** accept cards like `Kh` so the Super Tie color odds can use known colors.
