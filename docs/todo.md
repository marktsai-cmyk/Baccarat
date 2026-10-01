# To do

## Done

- ~~Configurable rebate~~: see [done.md](done.md#configurable-rebate-srcbetsrs).
- ~~Player/Banker Pair, Super 6~~: see [done.md](done.md#bets-and-payouts-srcbetsrs).
- ~~REST API~~: see [done.md](done.md#rest-api-server).

## Next for the REST API

- **Deploy:** a Dockerfile and CI pipeline, and a health check wired to `GET /health`.
- **Logging:** request logging (for example `tower-http`'s trace layer), so slow or failing requests show up.
- **Limits under load:** a cap on requests running at once, and a timeout per request.
- **A page for the API:** serve a viewer (for example Scalar or Swagger UI) for `openapi.yaml`.
- **Reload payouts:** pick up `baccarat.toml` changes without restarting.

## Other ideas

- **Rebate on turnover caps:** some casinos pay the rebate only on turnover above a minimum, or cap it per session. The simulator could model this.
- **Many shoes:** a `--shoes N` option that simulates many shoes in parallel and totals the results, to compare with the exact EV. Simulation stays internal: command line only, not the API.
- **EV during a shoe:** show each bet's exact EV before every simulated hand, to see how the remaining cards change it.
- **No-commission Banker:** the Banker bet that goes with Super 6 (1:1 with no commission, but a Banker win with 6 pays 1:2). `CoupResult::banker_six` already holds what it needs.
- **More side bets:** Lucky 6 (different pays for a 2-card and 3-card 6, needs the Banker's card count in `CoupResult`), Either Pair, Perfect Pair, Dragon Bonus.
- **Faster analysis:** the side-bet tracking made analysis about 1.5× slower (0.6 ms → 1.1 ms). Grouping deals that give the same hand totals could win this back.
- **Suits in `--dealt`:** accept cards like `Kh` so the Super Tie color odds can use known colors.
