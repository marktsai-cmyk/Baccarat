# To do

## 1. Configurable rebate

Many casinos give back a share of the amount bet (a rolling or turnover rebate, for example 0.8%). A rebate can turn a small negative EV into a positive one, so it belongs in the EV.

- Add a rebate rate to `Payouts` and a `--rebate RATE` option.
- Support the common versions, set by option:
  - on every bet: EV + rate
  - on losing bets only: EV + rate × P(lose)
  - counting only decided bets, so a Banker/Player bet that pushes on a tie earns nothing: EV + rate × (1 - P(push))
- Allow a different rate per bet, since some casinos pay no rebate on side bets.
- Show EV with and without the rebate, and add the rebate to the simulator's net results.
- Test: with a rebate on every bet, EV goes up by exactly the rate. On losing bets only, EV goes up by rate × P(lose).

## 2. REST API

Serve the calculator and simulator over HTTP so other apps can use them.

- Endpoints (JSON):
  - `POST /analyze`: body `{ "decks": 8, "dealt": ["K", "5"], "payouts": {...} }`, returns each bet's win, lose, push, EV and win rate excluding ties.
  - `POST /simulate`: body `{ "decks": 8, "cut": 14, "seed": 2026, "payouts": {...} }`, returns every hand and a summary.
  - `GET /health`
- Keep the library as it is and add the server as a separate binary or crate, so the command-line tool keeps working.
- Choose a web framework (for example axum on tokio). This adds the project's first outside dependencies.
- Reject bad input with clear 400 errors: too few or too many cards, unknown card, cut card out of range.
- An analysis takes about 1 ms. Cache results for repeated shoes, and put a limit on request sizes.
- Add an OpenAPI description and integration tests for each endpoint.

## Other ideas

- **Many shoes:** a `--shoes N` option that simulates many shoes in parallel and totals the results, to compare with the exact EV.
- **EV during a shoe:** show each bet's exact EV before every simulated hand, to see how the remaining cards change it.
- **More side bets:** Player/Banker Pair, Super 6 / no-commission Banker, Dragon Bonus.
- **Faster analysis:** the side-bet tracking made analysis about 1.5× slower (0.6 ms → 1.1 ms). Grouping deals that give the same hand totals could win this back.
- **Suits in `--dealt`:** accept cards like `Kh` so the Super Tie color odds can use known colors.
