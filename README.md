# Baccarat EV

Exact win rates and expected values (EV) for Punto Banco baccarat bets, from the command line or over a REST API, plus a shoe simulator for internal use (command line only).

Bets covered: **Banker**, **Player**, **Tie**, **Big**, **Small**, **Super Tie**, **Player Pair**, **Banker Pair** and **Super 6**.

## Quick start

Needs Rust 1.85 or newer (edition 2024). The calculator and command-line tool have no outside dependencies. Only the REST API server (`server/`) uses outside crates (axum, tokio, serde).

```bash
cargo run --release                                   # exact odds for a fresh 8-deck shoe
cargo run --release -- --decks 6                      # 6-deck shoe
cargo run --release -- --dealt K 5 A 9 10 6           # odds after these cards were dealt
cargo run --release -- simulate                       # deal one full shoe, random seed
cargo run --release -- simulate --seed 2026 --cut 16  # replay a shoe, cut card 16 from the end
cargo run --release -p baccarat-server                # REST API on http://127.0.0.1:3000
cargo test --release --workspace                      # run all the tests
```

## Example output (8 decks)

```
Bet                 Win      Lose      Push   EV / unit   Win rate excl. ties
Banker         45.8597%  44.6247%   9.5156%    -1.0579%              50.6825%
Player         44.6247%  45.8597%   9.5156%    -1.2351%              49.3175%
Tie             9.5156%  90.4844%   0.0000%   -14.3596%                     -
Big            62.1132%  37.8868%   0.0000%    -4.3457%                     -
Small          37.8868%  62.1132%   0.0000%    -5.2829%                     -
Super Tie       9.5156%  90.4844%   0.0000%    -5.9716%                     -
Player Pair     7.4699%  92.5301%   0.0000%   -10.3614%                     -
Banker Pair     7.4699%  92.5301%   0.0000%   -10.3614%                     -
Super 6         5.3864%  94.6136%   0.0000%   -29.9772%                     -
```

## Payouts file

Payouts and the rebate are set in [baccarat.toml](baccarat.toml). The tool reads that file from the folder it runs in, or the file given with `--config FILE`. With no file, it uses the built-in defaults, which are the values in the shipped `baccarat.toml`. Command-line options override the file for one run, and the first lines of the output say where the payouts came from.

```toml
tie = 9                      # payouts are N for N:1
big = 0.5
super_six = 15

[rebate]
on = "decided"               # "all", "losses" or "decided"
rate = 0                     # rate for every bet
banker = 0.008               # a bet's own line overrides `rate`
player = 0.008
```

Leave a line out to keep its default. A mistake stops the run with the line number, for example `baccarat.toml: line 2: unknown setting "super_6"`.

```bash
cargo run --release -- --config macau.toml
cargo run --release -- simulate --config macau.toml --tie-pays 8
```

## Options

| Option | Mode | Meaning | Default |
|---|---|---|---|
| `--decks N` | both | Decks in the shoe | 8 |
| `--dealt CARD...` | analyze | Cards already dealt (A, 2-9, 10/T, J, Q, K) | none |
| `--cut N` | simulate | Cut card position, in cards from the end (11 or more) | 14 |
| `--seed N` | simulate | Seed that replays a shoe | random |
| `--config FILE` | both | Read payouts and the rebate from FILE | `baccarat.toml` if present |
| `--commission RATE` | both | Banker commission | 0.05 |
| `--tie-pays N` | both | Tie pays N:1 | 8 |
| `--super-tie-pays S,M,O` | both | Super Tie payouts (see below) | 100,25,7 |
| `--pair-pays N` | both | Player Pair and Banker Pair pay N:1 | 11 |
| `--super-6-pays N` | both | Super 6 pays N:1 | 12 |
| `--rebate RATE` | both | Pay back RATE of every bet, e.g. `0.008` for 0.8% | 0 |
| `--rebate BET=RATE,...` | both | A rate per bet (others get none), e.g. `banker=0.008,player=0.008`. Bets: banker, player, tie, big, small, super-tie, player-pair, banker-pair, super-6 | none |
| `--rebate-on all\|losses\|decided` | both | Which bets earn the rebate: every bet, only losing bets, or every bet that doesn't push | all |

With a rebate, the analysis adds **Rebate** and **EV + rebate** columns, and the simulator shows the bet result, the rebate and the total separately. Win, lose and push are still based on the bet itself, so a rebate never turns a push into a win.

```bash
cargo run --release -- --rebate banker=0.008,player=0.008 --rebate-on decided
```

## REST API

```bash
cargo run --release -p baccarat-server -- --addr 127.0.0.1:3000 --config baccarat.toml
```

Both options are optional. Like the command-line tool, the server reads `baccarat.toml` from the folder it runs in. Its payouts apply to every request that doesn't change them.

| Endpoint | Does |
|---|---|
| `GET /health` | `{ "status": "ok" }` |
| `GET /openapi.yaml` | The full API description ([server/openapi.yaml](server/openapi.yaml)) |
| `POST /analyze` | Exact odds and EV of every bet for the next hand |

The simulator is for internal use, so it isn't part of the API. Run it with `cargo run --release -- simulate`.

Probabilities and EVs are fractions (0.458597 = 45.8597%). Every field of a request is optional, so `{}` works.

```bash
curl -X POST http://127.0.0.1:3000/analyze -H "Content-Type: application/json" \
  -d '{"decks": 8, "dealt": ["K", "5"], "payouts": {"super_six": 15, "rebate": {"on": "decided", "rates": {"banker": 0.008}}}}'
```

The answer, shortened and rounded:

```json
{
  "shoe": { "decks": 8, "dealt": 2, "cards_left": 414 },
  "payouts": { "banker_commission": 0.05, "tie": 8.0, "super_six": 15.0, "rebate": { "on": "decided", "rates": { "banker": 0.008, "player": 0.0 } } },
  "bets": [
    { "bet": "banker", "name": "Banker", "win": 0.458551, "lose": 0.446266, "push": 0.095183,
      "ev": -0.010643, "rebate": 0.007239, "ev_with_rebate": -0.003404, "win_rate_excluding_ties": 0.506789 }
  ],
  "super_tie": { "same_color": 0.000265, "mixed_color": 0.008565, "other": 0.086353 }
}
```

The full answer lists all 9 bets, and every payout and rebate rate.

- **Payouts in a request:** a request's `payouts` change only the fields they name. Giving a rebate `rate` or `rates` replaces the server's rebate rates.
- **Errors:** they come back as `{ "error": "..." }`. Bad input (an unknown card, field or bet, or a value out of range) gets 400, a body over 64 KB gets 413, and a body not sent as JSON gets 415.
- **Caching:** odds are cached per shoe, so repeating a shoe skips the 1 ms analysis.

## Bet rules

| Bet | Wins when | Pays |
|---|---|---|
| Banker | Banker hand is higher | 1:1 minus 5% commission; push on a tie |
| Player | Player hand is higher | 1:1; push on a tie |
| Tie | Hands are equal | 8:1 |
| Big | 5 or 6 cards are dealt | 0.54:1 |
| Small | Only 4 cards are dealt | 1.5:1 |
| Super Tie | Hands are equal (loses otherwise) | 100:1 six-card tie on 6-9, all one color; 25:1 same but mixed colors; 7:1 any other tie |
| Player Pair | Player's first two cards are the same rank (K-K yes, K-Q no) | 11:1 |
| Banker Pair | Banker's first two cards are the same rank | 11:1 |
| Super 6 | Banker wins with a total of 6 (2 or 3 cards) | 12:1 |

Super Tie follows the Bicycle Casino version documented by [Wizard of Odds](https://wizardofodds.com/games/baccarat/side-bets/super-tie/). Other casinos use the name for different bets. Super 6 is the side bet only; the no-commission Banker bet that usually goes with it (a Banker win with 6 pays 1:2) is not built yet.

## Project layout

```
src/rules.rs      cards, third-card rules, dealing one coup
src/bets.rs       bets and payouts
src/config.rs     reads payouts and the rebate from baccarat.toml
src/analysis.rs   exact odds by going through every possible deal
src/simulator.rs  shuffle, burn, deal a full shoe to the cut card
src/main.rs       command-line tool
server/           REST API (axum) for the odds: routes and JSON shapes in src/lib.rs, openapi.yaml, tests/api.rs
python/           the first version of the calculator, in Python
docs/             what is done and what is next
```

See [docs/done.md](docs/done.md) for how it works and how it was checked, and [docs/todo.md](docs/todo.md) for what is planned.
