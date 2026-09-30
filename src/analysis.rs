//! Exact odds of the next coup, found by walking every way it can be dealt.
//!
//! [`analyze`] walks every possible coup (P1, B1, P2, B2, optional P3, optional B3) and
//! weights each one by the number of ordered ways to draw those cards without replacement,
//! so the result is exact for any make-up: a fresh shoe or one that is partly dealt.

use crate::bets::{Bet, Payouts};
use crate::rules::{CoupResult, Outcome, SuperTie, banker_draws, player_draws, super_tie_by_color, winner};

/// Cards per deck by point value (0 = 10/J/Q/K, 1 = Ace, 2-9 = face value).
pub const CARDS_PER_DECK: [u32; 10] = [16, 4, 4, 4, 4, 4, 4, 4, 4, 4];

/// A shoe's make-up by point value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shoe {
    /// Cards the shoe started with; each value is half red and half black.
    pub full: [u32; 10],
    /// Cards still in the shoe.
    pub remaining: [u32; 10],
}

impl Shoe {
    /// A freshly shuffled shoe of `decks` decks.
    pub fn new(decks: u32) -> Self {
        Self::from_counts(CARDS_PER_DECK.map(|count| count * decks))
    }

    /// A fresh shoe holding `counts[v]` cards worth `v` points, half of each value red.
    pub fn from_counts(counts: [u32; 10]) -> Self {
        Self { full: counts, remaining: counts }
    }

    pub fn cards_left(&self) -> u32 {
        self.remaining.iter().sum()
    }

    /// Takes out a dealt card, given by rank such as "K", "A", "7" or "10".
    pub fn remove(&mut self, card: &str) -> Result<(), String> {
        let value = card_points(card).ok_or_else(|| format!("unknown card {card:?}"))?;
        if self.remaining[value] == 0 {
            return Err(format!("no card worth {value} left to remove for {card:?}"));
        }
        self.remaining[value] -= 1;
        Ok(())
    }

    /// `red_odds[v][k]`: chance that a card worth `v` is red when the `k` cards worth `v` dealt
    /// before it in the same coup were red. The colors of cards already taken out of the shoe
    /// are unknown, so by symmetry only the full shoe matters.
    fn red_odds(&self) -> [[f64; 6]; 10] {
        self.full.map(|cards| {
            let mut odds = [0.0; 6];
            for (earlier, chance) in (0u32..).zip(&mut odds) {
                if earlier < cards {
                    *chance = f64::from((cards / 2).saturating_sub(earlier)) / f64::from(cards - earlier);
                }
            }
            odds
        })
    }
}

/// Probability that six cards with these point values are all one color.
fn same_color_probability(red_odds: &[[f64; 6]; 10], values: [usize; 6]) -> f64 {
    let mut earlier = [0; 10];
    let mut all_red = 1.0;
    for value in values {
        all_red *= red_odds[value][earlier[value]];
        earlier[value] += 1;
    }
    2.0 * all_red // all black is just as likely
}

/// Point value of a card rank such as "K", "A", "7" or "10".
pub fn card_points(card: &str) -> Option<usize> {
    match card.to_ascii_uppercase().as_str() {
        "A" => Some(1),
        "T" | "10" | "J" | "Q" | "K" => Some(0),
        digits => digits.parse().ok().filter(|&value| value < 10),
    }
}

/// Odds of the next coup, as counts out of `total` equally likely ordered 6-card draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Odds {
    pub total: u64,
    /// Indexed by outcome (Banker, Player, Tie), then by cards dealt (4, 5, 6).
    pub counts: [[u64; 3]; 3],
    /// Six-card ties on 6-9: the ties the Super Tie bet pays by color.
    pub color_ties: u64,
    /// Expected number of those in which all six cards are one color.
    pub same_color_ties: f64,
}

/// Chances and expected value of a 1-unit bet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BetStats {
    pub win: f64,
    pub lose: f64,
    pub push: f64,
    /// Expected net units won, before any rebate.
    pub ev: f64,
    /// Expected rebate in units.
    pub rebate: f64,
}

impl BetStats {
    pub fn ev_with_rebate(&self) -> f64 {
        self.ev + self.rebate
    }
}

impl Odds {
    /// Probability of every distinct result that bets are settled on.
    pub fn results(&self) -> Vec<(CoupResult, f64)> {
        let total = self.total as f64;
        let mut results = Vec::new();
        for (outcome, by_cards) in [Outcome::Banker, Outcome::Player, Outcome::Tie].into_iter().zip(self.counts) {
            for (cards, count) in (4..=6).zip(by_cards) {
                let mut count = count as f64;
                if outcome == Outcome::Tie && cards == 6 {
                    let mixed = self.color_ties as f64 - self.same_color_ties;
                    count -= self.color_ties as f64;
                    results.push((CoupResult { outcome, cards, super_tie: Some(SuperTie::SameColorSixCard) }, self.same_color_ties / total));
                    results.push((CoupResult { outcome, cards, super_tie: Some(SuperTie::MixedColorSixCard) }, mixed / total));
                }
                let super_tie = (outcome == Outcome::Tie).then_some(SuperTie::OtherTie);
                results.push((CoupResult { outcome, cards, super_tie }, count / total));
            }
        }
        results
    }

    pub fn bet_stats(&self, bet: Bet, payouts: &Payouts) -> BetStats {
        let mut stats = BetStats::default();
        for (result, probability) in self.results() {
            let net = payouts.net_before_rebate(bet, &result);
            if net > 0.0 {
                stats.win += probability;
            } else if net < 0.0 {
                stats.lose += probability;
            } else {
                stats.push += probability;
            }
            stats.ev += probability * net;
            stats.rebate += probability * payouts.rebate(bet, net);
        }
        stats
    }

    fn add(&mut self, player_total: usize, banker_total: usize, cards: usize, count: u64) {
        self.counts[winner(player_total, banker_total) as usize][cards - 4] += count;
    }
}

/// Calls `f(remaining, value, ways)` for every card value still in the shoe, with one
/// card of that value taken out of `remaining` while `f` runs.
fn draw(remaining: &mut [u32; 10], mut f: impl FnMut(&mut [u32; 10], usize, u64)) {
    for value in 0..10 {
        let ways = remaining[value];
        if ways > 0 {
            remaining[value] -= 1;
            f(remaining, value, u64::from(ways));
            remaining[value] += 1;
        }
    }
}

/// Exact odds of the next coup dealt from `shoe`.
pub fn analyze(shoe: &Shoe) -> Result<Odds, String> {
    let cards = u64::from(shoe.cards_left());
    if cards < 6 {
        return Err(format!("a coup can use 6 cards, so the shoe must hold at least 6 (it holds {cards})"));
    }
    // Every coup is counted over N(N-1)...(N-5) ordered 6-card draws; if that fits in
    // a u64, so does every partial count below it.
    let total = (0..6)
        .try_fold(1u64, |product, drawn| product.checked_mul(cards - drawn))
        .ok_or_else(|| format!("a {cards}-card shoe is too large to count exactly in 64 bits"))?;
    // A coup that uses k cards is also weighted by the ordered ways to draw the
    // 6 - k cards it leaves unused, so every coup shares that denominator.
    let unused_after_4 = (cards - 4) * (cards - 5);
    let unused_after_5 = cards - 5;

    let red_odds = shoe.red_odds();

    let mut odds = Odds { total, counts: [[0; 3]; 3], color_ties: 0, same_color_ties: 0.0 };
    let mut remaining = shoe.remaining;
    draw(&mut remaining, |remaining, p1, ways1| {
        draw(remaining, |remaining, b1, ways2| {
            draw(remaining, |remaining, p2, ways3| {
                draw(remaining, |remaining, b2, ways4| {
                    let ways = ways1 * ways2 * ways3 * ways4;
                    let player = (p1 + p2) % 10;
                    let banker = (b1 + b2) % 10;
                    if player >= 8 || banker >= 8 {
                        // natural: both hands stand
                        odds.add(player, banker, 4, ways * unused_after_4);
                    } else if player_draws(player) {
                        draw(remaining, |remaining, p3, ways5| {
                            let player = (player + p3) % 10;
                            if banker_draws(banker, Some(p3)) {
                                draw(remaining, |_, b3, ways6| {
                                    let banker = (banker + b3) % 10;
                                    let count = ways * ways5 * ways6;
                                    odds.add(player, banker, 6, count);
                                    if player == banker && super_tie_by_color(6, player) {
                                        odds.color_ties += count;
                                        odds.same_color_ties += count as f64 * same_color_probability(&red_odds, [p1, b1, p2, b2, p3, b3]);
                                    }
                                });
                            } else {
                                odds.add(player, banker, 5, ways * ways5 * unused_after_5);
                            }
                        });
                    } else if banker_draws(banker, None) {
                        draw(remaining, |_, b3, ways5| {
                            odds.add(player, (banker + b3) % 10, 5, ways * ways5 * unused_after_5);
                        });
                    } else {
                        odds.add(player, banker, 4, ways * unused_after_4);
                    }
                });
            });
        });
    });
    Ok(odds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bets::{Rebate, RebateBasis};
    use crate::rules::{Card, Coup, Suit};

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!((actual - expected).abs() <= tolerance, "{actual} is not within {tolerance} of {expected}");
    }

    /// The physical cards of a shoe holding `counts[v]` cards worth `v`, half of each value red.
    fn physical_cards(counts: [u32; 10]) -> Vec<Card> {
        let mut cards = Vec::new();
        for (value, count) in (0u8..).zip(counts) {
            let rank = if value == 0 { 13 } else { value };
            for i in 0..count {
                let suit = if i % 2 == 0 { Suit::Hearts } else { Suit::Spades };
                cards.push(Card { rank, suit });
            }
        }
        cards
    }

    /// Calls `f` with every ordered selection of `length` distinct cards from `cards`.
    fn for_each_selection(cards: &mut [Card], chosen: usize, length: usize, f: &mut impl FnMut(&[Card])) {
        if chosen == length {
            f(&cards[..length]);
            return;
        }
        for i in chosen..cards.len() {
            cards.swap(chosen, i);
            for_each_selection(cards, chosen + 1, length, f);
            cards.swap(chosen, i);
        }
    }

    #[test]
    fn matches_dealing_every_ordered_six_cards_of_a_small_shoe() {
        for counts in [[4, 2, 0, 0, 0, 2, 2, 0, 2, 0], [2, 0, 2, 2, 2, 0, 2, 2, 0, 0], [4, 0, 2, 0, 2, 0, 0, 2, 0, 2]] {
            // Every ordered draw of 6 distinct cards is equally likely and 6 cards always finish
            // a coup, so dealing each one physically gives exact odds to compare against.
            let mut dealt = Odds { total: 0, counts: [[0; 3]; 3], color_ties: 0, same_color_ties: 0.0 };
            for_each_selection(&mut physical_cards(counts), 0, 6, &mut |six| {
                let result = Coup::deal(six).result();
                dealt.total += 1;
                dealt.counts[result.outcome as usize][result.cards - 4] += 1;
                match result.super_tie {
                    Some(SuperTie::SameColorSixCard) => {
                        dealt.color_ties += 1;
                        dealt.same_color_ties += 1.0;
                    }
                    Some(SuperTie::MixedColorSixCard) => dealt.color_ties += 1,
                    _ => {}
                }
            });
            let analyzed = analyze(&Shoe::from_counts(counts)).unwrap();
            assert_eq!((analyzed.total, analyzed.counts, analyzed.color_ties), (dealt.total, dealt.counts, dealt.color_ties), "{counts:?}");
            assert_close(analyzed.same_color_ties, dealt.same_color_ties, 1e-6);
        }
    }

    #[test]
    fn eight_deck_shoe_matches_published_values() {
        let odds = analyze(&Shoe::new(8)).unwrap();
        // Wizard of Odds: exact counts out of 416 * 415 * 414 * 413 * 412 * 411 ordered draws
        assert_eq!(odds.total, 4_998_398_275_503_360);
        let by_outcome = odds.counts.map(|by_cards| by_cards.iter().sum::<u64>());
        assert_eq!(by_outcome, [2_292_252_566_437_888, 2_230_518_282_592_256, 475_627_426_473_216]);

        let total = odds.total as f64;
        assert_close(odds.same_color_ties / total, 0.00026543, 5e-9);
        assert_close((odds.color_ties as f64 - odds.same_color_ties) / total, 0.00857508, 2e-8);

        let payouts = Payouts::default();
        let ev = |bet| odds.bet_stats(bet, &payouts).ev;
        assert_close(ev(Bet::Banker), -0.010579, 5e-7);
        assert_close(ev(Bet::Player), -0.012351, 5e-7);
        assert_close(ev(Bet::Tie), -0.143596, 5e-7);
        assert_close(ev(Bet::SuperTie), -0.059716, 5e-6);
    }

    #[test]
    fn six_deck_big_and_small_match_published_values() {
        // Wizard of Odds: Small wins 37.8925% (house edge 5.27%), Big 62.1075% (house edge
        // 4.35445%, worked out from that rounded win rate)
        let odds = analyze(&Shoe::new(6)).unwrap();
        let payouts = Payouts::default();
        let small = odds.bet_stats(Bet::Small, &payouts);
        let big = odds.bet_stats(Bet::Big, &payouts);
        assert_close(small.win, 0.378925, 5e-7);
        assert_close(big.win, 0.621075, 5e-7);
        assert_close(small.ev, -0.0527, 5e-5);
        assert_close(big.ev, -0.0435445, 1e-6);
    }

    #[test]
    fn rebate_adds_to_ev_on_the_chosen_bets() {
        let odds = analyze(&Shoe::new(8)).unwrap();
        let rate = 0.008;
        for bet in Bet::ALL {
            let plain = odds.bet_stats(bet, &Payouts::default());
            let with_rebate = |basis| {
                let payouts = Payouts { rebate: Rebate { rates: [rate; 6], basis }, ..Payouts::default() };
                let stats = odds.bet_stats(bet, &payouts);
                assert_eq!((stats.win, stats.lose, stats.push, stats.ev), (plain.win, plain.lose, plain.push, plain.ev));
                stats.ev_with_rebate()
            };
            assert_close(with_rebate(RebateBasis::AllBets), plain.ev + rate, 1e-12);
            assert_close(with_rebate(RebateBasis::Losses), plain.ev + rate * plain.lose, 1e-12);
            assert_close(with_rebate(RebateBasis::DecidedBets), plain.ev + rate * (1.0 - plain.push), 1e-12);
        }
    }

    #[test]
    fn rejects_shoes_it_cannot_count() {
        assert!(analyze(&Shoe::from_counts([5, 0, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        assert!(analyze(&Shoe::new(40)).is_err());
    }

    #[test]
    fn removes_dealt_cards_by_rank() {
        let mut shoe = Shoe::new(1);
        for card in ["K", "10", "a", "9"] {
            shoe.remove(card).unwrap();
        }
        assert_eq!(shoe.remaining, [14, 3, 4, 4, 4, 4, 4, 4, 4, 3]);
        assert_eq!(shoe.full, Shoe::new(1).full);
        assert!(shoe.remove("X").is_err());
        assert!(Shoe::from_counts([0; 10]).remove("5").is_err());
    }
}
