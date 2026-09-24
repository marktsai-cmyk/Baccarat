//! The bets on the table and what they pay.

use crate::rules::{CoupResult, Outcome, SuperTie};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bet {
    Banker,
    Player,
    Tie,
    Big,
    Small,
    SuperTie,
}

impl Bet {
    pub const ALL: [Bet; 6] = [Bet::Banker, Bet::Player, Bet::Tie, Bet::Big, Bet::Small, Bet::SuperTie];

    pub fn name(self) -> &'static str {
        match self {
            Bet::Banker => "Banker",
            Bet::Player => "Player",
            Bet::Tie => "Tie",
            Bet::Big => "Big",
            Bet::Small => "Small",
            Bet::SuperTie => "Super Tie",
        }
    }
}

/// What each bet pays, as N for N:1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Payouts {
    /// Share of a Banker win kept by the house, e.g. 0.05.
    pub banker_commission: f64,
    pub tie: f64,
    /// Big wins when 5 or 6 cards are dealt.
    pub big: f64,
    /// Small wins when only 4 cards are dealt.
    pub small: f64,
    /// Super Tie: same-color six-card tie on 6-9, mixed-color one, any other tie.
    pub super_tie: [f64; 3],
}

impl Default for Payouts {
    fn default() -> Self {
        // Big/Small as dealt by Playtech; Super Tie as at the Bicycle Casino.
        Self { banker_commission: 0.05, tie: 8.0, big: 0.54, small: 1.5, super_tie: [100.0, 25.0, 7.0] }
    }
}

impl Payouts {
    /// Net units won (negative when lost) by a 1-unit `bet` on a coup that ended in `result`.
    pub fn net(&self, bet: Bet, result: &CoupResult) -> f64 {
        let pays_if = |won: bool, payout: f64| if won { payout } else { -1.0 };
        match bet {
            Bet::Banker => match result.outcome {
                Outcome::Banker => 1.0 - self.banker_commission,
                Outcome::Player => -1.0,
                Outcome::Tie => 0.0,
            },
            Bet::Player => match result.outcome {
                Outcome::Player => 1.0,
                Outcome::Banker => -1.0,
                Outcome::Tie => 0.0,
            },
            Bet::Tie => pays_if(result.outcome == Outcome::Tie, self.tie),
            Bet::Big => pays_if(result.cards >= 5, self.big),
            Bet::Small => pays_if(result.cards == 4, self.small),
            Bet::SuperTie => match result.super_tie {
                Some(SuperTie::SameColorSixCard) => self.super_tie[0],
                Some(SuperTie::MixedColorSixCard) => self.super_tie[1],
                Some(SuperTie::OtherTie) => self.super_tie[2],
                None => -1.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settles_every_bet() {
        let payouts = Payouts::default();
        let nets = |result: CoupResult| Bet::ALL.map(|bet| payouts.net(bet, &result));

        let banker_natural = CoupResult { outcome: Outcome::Banker, cards: 4, super_tie: None };
        assert_eq!(nets(banker_natural), [0.95, -1.0, -1.0, -1.0, 1.5, -1.0]);

        let player_five_cards = CoupResult { outcome: Outcome::Player, cards: 5, super_tie: None };
        assert_eq!(nets(player_five_cards), [-1.0, 1.0, -1.0, 0.54, -1.0, -1.0]);

        let color_tie = CoupResult { outcome: Outcome::Tie, cards: 6, super_tie: Some(SuperTie::SameColorSixCard) };
        assert_eq!(nets(color_tie), [0.0, 0.0, 8.0, 0.54, -1.0, 100.0]);

        let mixed_tie = CoupResult { super_tie: Some(SuperTie::MixedColorSixCard), ..color_tie };
        assert_eq!(payouts.net(Bet::SuperTie, &mixed_tie), 25.0);

        let four_card_tie = CoupResult { outcome: Outcome::Tie, cards: 4, super_tie: Some(SuperTie::OtherTie) };
        assert_eq!(nets(four_card_tie), [0.0, 0.0, 8.0, -1.0, 1.5, 7.0]);
    }
}
