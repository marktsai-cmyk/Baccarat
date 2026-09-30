# What is done

## Exact odds calculator (`src/analysis.rs`)

- **Shoe model:** 10 counts, one per point value (0 = 10/J/Q/K, 1 = Ace, 2-9 = face value). Works for a fresh shoe or one with cards already dealt (`--dealt`).
- **Method:** goes through every possible deal in dealing order (P1, B1, P2, B2, then the third-card rules), taking each card out of the shoe as it is drawn. Each deal is weighted by the number of ways its cards can be drawn.
- **Exact counts:** a hand uses 4, 5 or 6 cards. 4- and 5-card hands are also multiplied by the ways to draw the unused cards, so every hand is counted out of the same total, N(N-1)...(N-5). Counts are whole 64-bit numbers, so results are exact for shoes of up to 31 decks. Larger shoes return an error.
- **Results tracked:** winner (Banker/Player/Tie) by number of cards dealt (4/5/6), plus six-card ties on 6-9 and how many of those are one color.
- **Super Tie colors:** the full shoe is half red and half black for every value. Cards removed with `--dealt` are given by rank only, so their colors are unknown. With that, the chance that six cards of given values are all one color depends only on the full shoe, and the result stays exact.
- **Speed:** about 1.1 ms per 8-deck analysis in release builds. The Python version takes about 160 ms.

## Bets and payouts (`src/bets.rs`)

- Banker (5% commission), Player, Tie (8:1), Big (0.54:1), Small (1.5:1), Super Tie (100/25/7).
- `Payouts::net` settles one bet on one hand. Both the analyzer (for EV) and the simulator (for results) use it, so they always agree.
- The commission, Tie payout and Super Tie table can be changed from the command line.

## Simulator (`src/simulator.rs`)

- Builds a shoe of real cards (rank and suit) and shuffles it with Fisher-Yates.
- The random number generator is xoshiro256** seeded through SplitMix64, with unbiased range sampling (Lemire's method). There are no outside crates, and the same seed always replays the same shoe.
- The first card is turned over and that many more are burned (10/J/Q/K burn 10).
- The cut card sits `--cut` cards from the end (default 14, at least 11). When it comes out, the current hand is finished and one more is dealt.
- Prints every hand, then counts and the net result of betting 1 unit on every bet each hand.
- About 30 µs per 8-deck shoe (around 81 hands).

## How it was checked

15 tests, run with `cargo test --release`:

- **Brute-force deal:** for three small 12-card shoes with real suits, every ordered six-card draw (665,280 each) is dealt physically. The results match the analyzer exactly, including the color counts.
- **Published figures (Wizard of Odds):**
  - 8 decks: Banker, Player and Tie counts match to the last digit (for example 2,292,252,566,437,888 Banker wins out of 4,998,398,275,503,360 possible deals). Banker EV -1.0579%, Player -1.2351%, Tie -14.3596%.
  - Super Tie, 8 decks: 0.026543% same color, 0.857508% mixed colors, house edge 5.97% (10.26% with the 100/20/7 table).
  - Big/Small, 6 decks: Small wins 37.8925%, Big 62.1075%.
- **Rules:** the Banker third-card rule is checked against the full drawing table. Hand-built hands check naturals and the Super Tie color rule.
- **Simulator:** burn count, the rule of one more hand after the cut card, the shuffle keeping every card, replay by seed, and bad settings rejected.

## Python version (`python/`)

The first version of the calculator: Banker, Player and Tie only, with exact fractions. It gives the same numbers as the Rust version. Run its tests with `python -m unittest discover -s python`.
