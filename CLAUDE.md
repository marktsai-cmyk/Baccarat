# CLAUDE.md

Baccarat (Punto Banco) odds in Rust: exact win rates and EV of Banker, Player, Tie, Big, Small, Super Tie, the pairs and Super 6, from the command line and a REST API, plus a full-shoe simulator for internal use (command line only). See README.md for usage, docs/done.md for what exists, and docs/todo.md for what is next.

## Commands

```bash
cargo test --release --workspace                      # all tests; release, because the brute-force test is slow in debug
cargo clippy --release --workspace --all-targets      # must stay warning-free
cargo run --release -- [analyze options]              # exact odds (default mode)
cargo run --release -- simulate [options]             # deal one shoe
cargo run --release -p baccarat-server -- [--addr HOST:PORT] [--config FILE]   # REST API
python -m unittest discover -s python                 # tests for the old Python version
```

## Layout

- `src/rules.rs`: `Card`, `Coup::deal`, third-card rules, `CoupResult` (what bets settle on)
- `src/bets.rs`: `Bet`, `Payouts`, `Rebate`, and `Payouts::net` (= `net_before_rebate` + `rebate`), the single source of truth for settling a bet. Classify win/lose/push with `net_before_rebate`, never with `net`.
- `src/config.rs`: reads `baccarat.toml` into `Payouts`. A new payout setting goes in `Payouts::default()`, `parse_payouts` and `baccarat.toml` (a test keeps the file and the defaults equal).
- `src/analysis.rs`: `Shoe` (counts by point value), `analyze`, `Odds`
- `src/simulator.rs`: shuffle, burn, cut card, and the built-in xoshiro256** random number generator
- `src/main.rs`: command-line parsing and output only, no game logic
- `server/` (workspace member `baccarat-server`): the REST API on axum. `src/lib.rs` has the routes, the request and response types (serde), the per-shoe odds cache and `ApiError`. `src/main.rs` only starts it. `openapi.yaml` describes the API and must be updated with any change to the routes or JSON shapes. `tests/api.rs` calls the routes in-process with `tower::ServiceExt::oneshot`.

## Rules to keep

- **No outside crates** in the root crate (library and command-line tool). They belong only in `server/`. Don't add serde derives to library types: the server maps them to its own JSON types, so the library and the API can change separately.
- **Simulation is internal:** the simulator is for internal use only, so keep it in the command-line tool. Don't add simulation endpoints to the REST API (a test checks that `/simulate` gives 404).
- **API input is strict:** request types use `deny_unknown_fields`, payouts go through `Payouts::validate`, and every error is `ApiError` (JSON `{ "error" }`; 400 for bad input, 413/415 kept from axum).
- **Exact analysis:** counts stay as whole `u64` numbers over the common total N(N-1)...(N-5). Only the Super Tie color share is `f64`. Keep the overflow check in `analyze`.
- **One settlement path:** a new bet goes in `Bet` and `Payouts::net`. Both the analyzer and the simulator settle through it, so don't add payout math anywhere else.
- **New bets need `CoupResult` data:** if a bet depends on something `CoupResult` doesn't hold, add it there, and make `analyze` tally it and `Odds::results` report it. Pair bets are the exception: they depend only on a hand's first two cards, so `Odds::pair_probability` is worked out directly and `bet_stats` settles them on pair / no pair.
- **Tests check against outside numbers:** new bets need a brute-force check (`matches_dealing_every_ordered_six_cards_of_a_small_shoe`) and, when possible, a published figure (Wizard of Odds). If a published figure was rounded, allow for that rounding in the test instead of weakening the check.
- **Simulator:** the cut card must stay at least 11 cards from the end (`MIN_CUT`), and the same seed must always replay the same shoe.

## Working with the user

- "Design X" means describe the design in chat, with no code. Only implement when asked.
- Commit only when asked, and show the proposed message first.
