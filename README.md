# Baccarat EV

Exact win rates and expected values (EV) for Punto Banco baccarat bets, plus a simulator that deals a full shoe.

Bets covered: **Banker**, **Player**, **Tie**, **Big**, **Small** and **Super Tie**.

## Quick start

Needs Rust 1.85 or newer (edition 2024). There are no outside dependencies.

```bash
cargo run --release                                   # exact odds for a fresh 8-deck shoe
cargo run --release -- --decks 6                      # 6-deck shoe
cargo run --release -- --dealt K 5 A 9 10 6           # odds after these cards were dealt
cargo run --release -- simulate                       # deal one full shoe, random seed
cargo run --release -- simulate --seed 2026 --cut 16  # replay a shoe, cut card 16 from the end
cargo test --release                                  # run the tests
```

## Example output (8 decks)

```
Bet               Win      Lose      Push   EV / unit   Win rate excl. ties
Banker       45.8597%  44.6247%   9.5156%    -1.0579%              50.6825%
Player       44.6247%  45.8597%   9.5156%    -1.2351%              49.3175%
Tie           9.5156%  90.4844%   0.0000%   -14.3596%                     -
Big          62.1132%  37.8868%   0.0000%    -4.3457%                     -
Small        37.8868%  62.1132%   0.0000%    -5.2829%                     -
Super Tie     9.5156%  90.4844%   0.0000%    -5.9716%                     -
```

## Options

| Option | Mode | Meaning | Default |
|---|---|---|---|
| `--decks N` | both | Decks in the shoe | 8 |
| `--dealt CARD...` | analyze | Cards already dealt (A, 2-9, 10/T, J, Q, K) | none |
| `--cut N` | simulate | Cut card position, in cards from the end (11 or more) | 14 |
| `--seed N` | simulate | Seed that replays a shoe | random |
| `--commission RATE` | both | Banker commission | 0.05 |
| `--tie-pays N` | both | Tie pays N:1 | 8 |
| `--super-tie-pays S,M,O` | both | Super Tie payouts (see below) | 100,25,7 |

## Bet rules

| Bet | Wins when | Pays |
|---|---|---|
| Banker | Banker hand is higher | 1:1 minus 5% commission; push on a tie |
| Player | Player hand is higher | 1:1; push on a tie |
| Tie | Hands are equal | 8:1 |
| Big | 5 or 6 cards are dealt | 0.54:1 |
| Small | Only 4 cards are dealt | 1.5:1 |
| Super Tie | Hands are equal (loses otherwise) | 100:1 six-card tie on 6-9, all one color; 25:1 same but mixed colors; 7:1 any other tie |

Super Tie follows the Bicycle Casino version documented by [Wizard of Odds](https://wizardofodds.com/games/baccarat/side-bets/super-tie/). Other casinos use the name for different bets.

## Project layout

```
src/rules.rs      cards, third-card rules, dealing one coup
src/bets.rs       bets and payouts
src/analysis.rs   exact odds by going through every possible deal
src/simulator.rs  shuffle, burn, deal a full shoe to the cut card
src/main.rs       command-line tool
python/           the first version of the calculator, in Python
docs/             what is done and what is next
```

See [docs/done.md](docs/done.md) for how it works and how it was checked, and [docs/todo.md](docs/todo.md) for what is planned.
