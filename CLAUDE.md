# CLAUDE.md

Baccarat (Punto Banco) odds in Rust: exact win rates and EV of Banker, Player, Tie, Big, Small and Super Tie, plus a full-shoe simulator. See README.md for usage, docs/done.md for what exists, and docs/todo.md for what is next.

## Commands

```bash
cargo test --release                            # all tests; release, because the brute-force test is slow in debug
cargo clippy --release --all-targets            # must stay warning-free
cargo run --release -- [analyze options]        # exact odds (default mode)
cargo run --release -- simulate [options]       # deal one shoe
python -m unittest discover -s python           # tests for the old Python version
```

## Layout

- `src/rules.rs`: `Card`, `Coup::deal`, third-card rules, `CoupResult` (what bets settle on)
- `src/bets.rs`: `Bet`, `Payouts`, and `Payouts::net`, the single source of truth for settling a bet
- `src/analysis.rs`: `Shoe` (counts by point value), `analyze`, `Odds`
- `src/simulator.rs`: shuffle, burn, cut card, and the built-in xoshiro256** random number generator
- `src/main.rs`: command-line parsing and output only, no game logic

## Rules to keep

- **No outside crates** in the library. If the REST API needs a web framework, keep it out of the core library.
- **Exact analysis:** counts stay as whole `u64` numbers over the common total N(N-1)...(N-5). Only the Super Tie color share is `f64`. Keep the overflow check in `analyze`.
- **One settlement path:** a new bet goes in `Bet` and `Payouts::net`. Both the analyzer and the simulator settle through it, so don't add payout math anywhere else.
- **New bets need `CoupResult` data:** if a bet depends on something `CoupResult` doesn't hold (pairs, the winning total), add it there, and make `analyze` tally it and `Odds::results` report it.
- **Tests check against outside numbers:** new bets need a brute-force check (`matches_dealing_every_ordered_six_cards_of_a_small_shoe`) and, when possible, a published figure (Wizard of Odds). If a published figure was rounded, allow for that rounding in the test instead of weakening the check.
- **Simulator:** the cut card must stay at least 11 cards from the end (`MIN_CUT`), and the same seed must always replay the same shoe.

## Working with the user

- "Design X" means describe the design in chat, with no code. Only implement when asked.
- Commit only when asked, and show the proposed message first.
