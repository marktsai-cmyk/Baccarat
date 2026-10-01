# What is done

## Exact odds calculator (`src/analysis.rs`)

- **Shoe model:** 10 counts, one per point value (0 = 10/J/Q/K, 1 = Ace, 2-9 = face value). Works for a fresh shoe or one with cards already dealt (`--dealt`).
- **Method:** goes through every possible deal in dealing order (P1, B1, P2, B2, then the third-card rules), taking each card out of the shoe as it is drawn. Each deal is weighted by the number of ways its cards can be drawn.
- **Exact counts:** a hand uses 4, 5 or 6 cards. 4- and 5-card hands are also multiplied by the ways to draw the unused cards, so every hand is counted out of the same total, N(N-1)...(N-5). Counts are whole 64-bit numbers, so results are exact for shoes of up to 31 decks. Larger shoes return an error.
- **Results tracked:** winner (Banker/Player/Tie) by number of cards dealt (4/5/6), Banker wins with 6 (for Super 6), plus six-card ties on 6-9 and how many of those are one color.
- **Super Tie colors:** the full shoe is half red and half black for every value. Cards removed with `--dealt` are counted by point value only, so their colors are unknown. With that, the chance that six cards of given values are all one color depends only on the full shoe, and the result stays exact.
- **Pairs:** a hand's first two cards are equally likely to be any two cards in the shoe, so the pair chance doesn't depend on the rest of the deal and is worked out directly: Σ n(n-1) / (N(N-1)) over the values. Cards worth 0 come in four ranks (10/J/Q/K) split evenly in the full shoe, so two of them match with chance (Z/4 - 1) / (Z - 1), where Z is the full shoe's count of 0s. For 8 decks this gives exactly 31/415 = 7.4699%.
- **Speed:** about 1.1 ms per 8-deck analysis in release builds. The Python version takes about 160 ms.

## Bets and payouts (`src/bets.rs`)

- Banker (5% commission), Player, Tie (8:1), Big (0.54:1), Small (1.5:1), Super Tie (100/25/7), Player Pair and Banker Pair (11:1, same rank), Super 6 (12:1, Banker wins with 6).
- `Payouts::net` settles one bet on one hand. Both the analyzer (for EV) and the simulator (for results) use it, so they always agree.
- The commission and the payouts of Tie, Super Tie, the pairs and Super 6 can be changed from the command line.

## Configurable rebate (`src/bets.rs`)

- `Payouts::rebate` holds a rate for each bet and a `RebateBasis`:
  - `AllBets`: every bet earns it, so EV + rate
  - `Losses`: only losing bets, so EV + rate × P(lose)
  - `DecidedBets`: every bet that doesn't push, so EV + rate × (1 - P(push)); a Banker/Player bet on a tie earns nothing
- `Payouts::net` = `net_before_rebate` + `rebate`. Win, lose and push come from `net_before_rebate`, so a rebate never turns a push into a win.
- `BetStats` reports `ev` (before rebate), `rebate` (expected rebate) and `ev_with_rebate()`.
- Command line: `--rebate 0.008`, or `--rebate banker=0.008,player=0.008` for per-bet rates, plus `--rebate-on all|losses|decided`. Rates must be from 0 up to (but not including) 1.
- Example: with 0.8% on decided Banker bets, Banker EV goes from -1.0579% to -0.3340%.

## Payouts file (`src/config.rs`, `baccarat.toml`)

- Every payout (including Big and Small) and the rebate can be set in a TOML file. `baccarat.toml` in the working folder is read by default, or the file given with `--config FILE`. Command-line options override the file.
- The parser understands only the TOML the file needs, so there are still no outside crates: `key = value`, `#` comments, a `[rebate]` section, and numbers, "strings" and [lists].
- In `[rebate]`, `rate` applies to every bet and a bet's own key (e.g. `banker`, `super-6`) overrides it. `on` is "all", "losses" or "decided".
- Unknown settings, repeated settings, bad values and out-of-range rates are rejected with the line number.
- The shipped `baccarat.toml` lists every setting with its default, and a test checks it still matches `Payouts::default()`.

## Simulator (`src/simulator.rs`)

- Builds a shoe of real cards (rank and suit) and shuffles it with Fisher-Yates.
- The random number generator is xoshiro256** seeded through SplitMix64, with unbiased range sampling (Lemire's method). There are no outside crates, and the same seed always replays the same shoe.
- The first card is turned over and that many more are burned (10/J/Q/K burn 10).
- The cut card sits `--cut` cards from the end (default 14, at least 11). When it comes out, the current hand is finished and one more is dealt.
- Prints every hand with the side bets it won, then counts and the net result of betting 1 unit on every bet each hand.
- About 30 µs per 8-deck shoe (around 81 hands).

## REST API (`server/`)

- A separate crate, `baccarat-server`, in a Cargo workspace with the library. It uses axum 0.8 on tokio with serde, so the outside crates stay out of the library and the command-line tool.
- `GET /health`, `GET /openapi.yaml` (the API description, [server/openapi.yaml](../server/openapi.yaml)) and `POST /analyze`. See the README for request and answer examples.
- **No simulation:** the simulator is for internal use, so it stays in the command-line tool and isn't served. A test checks that `/simulate` gives 404 and that the API description doesn't mention it.
- **Payouts:** the server reads `baccarat.toml` (or `--config FILE`) at start-up. A request's `payouts` change only the fields they name, and giving a rebate `rate` or `rates` replaces the server's rebate rates. Every answer echoes the payouts it used.
- **Input checks:** request types reject unknown fields. Decks must be 1-31, and payouts go through `Payouts::validate`. Every error is JSON `{ "error": "..." }`: 400 for bad input, 413 for a body over 64 KB, 415 for a body not sent as JSON.
- **Cache:** odds are cached per shoe (up to 10,000 shoes, then the cache starts over), since payouts are applied after the analysis. A cache miss runs the analysis on a blocking thread, so it doesn't hold up other requests.
- **JSON types:** the request and answer types live in the server crate, so the library has no serde derives.
- `Payouts::validate` was added to the library for the server, and the command-line tool now uses it too (it now rejects `--commission 5`).

## How it was checked

31 tests, run with `cargo test --release --workspace` (23 in the library, 8 in the API):

- **Brute-force deal:** for three small 12-card shoes with real ranks and suits, every ordered six-card draw (665,280 each) is dealt physically. The results match the analyzer exactly: outcomes, card counts, Banker wins with 6, color counts and both pair chances.
- **Published figures (Wizard of Odds):**
  - 8 decks: Banker, Player and Tie counts match to the last digit (for example 2,292,252,566,437,888 Banker wins out of 4,998,398,275,503,360 possible deals). Banker EV -1.0579%, Player -1.2351%, Tie -14.3596%.
  - Super Tie, 8 decks: 0.026543% same color, 0.857508% mixed colors, house edge 5.97% (10.26% with the 100/20/7 table).
  - Big/Small, 6 decks: Small wins 37.8925%, Big 62.1075%.
  - Pairs, 8 decks: 31/415 = 7.4699%, house edge 10.36% at 11:1.
  - Super 6, 8 decks: Banker wins with 6 5.3864% of the time, house edge 29.98% at 12:1 (13.82% at 15:1).
- **Rules:** the Banker third-card rule is checked against the full drawing table. Hand-built hands check naturals, the Super Tie color rule, pairs by rank (K-K yes, K-Q no) and a Banker win with 6.
- **Rebate:** each basis pays the right bets on a hand-built tie. For every bet on an 8-deck shoe, EV with the rebate equals EV + rate, EV + rate × P(lose) or EV + rate × (1 - P(push)), and win/lose/push don't change.
- **Payouts file:** the shipped file equals the defaults, a sample file sets payouts and per-bet rebates, and eight kinds of mistake give the right error and line number.
- **Payout checks:** `Payouts::validate` accepts the defaults and rejects a 100% commission, a negative payout, a NaN Super Tie payout and a rebate rate over 1.
- **REST API** (`server/tests/api.rs`, routes called in-process): health; the OpenAPI file lists every route and bet; a fresh shoe matches the published Banker, pair, Super 6 and Super Tie figures; request payouts change only what they name; the cache keeps one entry per shoe whatever the payouts; `/simulate` is not served; ten kinds of bad input each get a 400 with the right message; a body not sent as JSON gets 415, and one over 64 KB gets 413.
- **Simulator:** burn count, the rule of one more hand after the cut card, the shuffle keeping every card, replay by seed, and bad settings rejected.

## Python version (`python/`)

The first version of the calculator: Banker, Player and Tie only, with exact fractions. It gives the same numbers as the Rust version. Run its tests with `python -m unittest discover -s python`.
