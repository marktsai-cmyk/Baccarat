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
    PlayerPair,
    BankerPair,
    SuperSix,
}

impl Bet {
    pub const ALL: [Bet; 9] = [
        Bet::Banker,
        Bet::Player,
        Bet::Tie,
        Bet::Big,
        Bet::Small,
        Bet::SuperTie,
        Bet::PlayerPair,
        Bet::BankerPair,
        Bet::SuperSix,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Bet::Banker => "Banker",
            Bet::Player => "Player",
            Bet::Tie => "Tie",
            Bet::Big => "Big",
            Bet::Small => "Small",
            Bet::SuperTie => "Super Tie",
            Bet::PlayerPair => "Player Pair",
            Bet::BankerPair => "Banker Pair",
            Bet::SuperSix => "Super 6",
        }
    }

    /// Name used in options and config files, e.g. "super-tie".
    pub fn key(self) -> String {
        self.name().to_ascii_lowercase().replace(' ', "-")
    }

    pub fn from_key(key: &str) -> Option<Bet> {
        Bet::ALL.into_iter().find(|bet| bet.key() == key.trim().to_ascii_lowercase())
    }
}

/// Which bets earn the rebate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebateBasis {
    /// Every bet, win or lose.
    AllBets,
    /// Only bets that lose.
    Losses,
    /// Every bet that doesn't push, so a Banker or Player bet on a tie earns nothing.
    DecidedBets,
}

/// Share of the amount bet that the house pays back, on top of what the bet itself pays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rebate {
    /// Rate for each bet, in the order of `Bet::ALL`, e.g. 0.008 for 0.8%.
    pub rates: [f64; Bet::ALL.len()],
    pub basis: RebateBasis,
}

impl Rebate {
    pub const NONE: Rebate = Rebate { rates: [0.0; Bet::ALL.len()], basis: RebateBasis::AllBets };

    pub fn is_none(&self) -> bool {
        self.rates.iter().all(|&rate| rate == 0.0)
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
    /// Player Pair and Banker Pair: the hand's first two cards are the same rank.
    pub pair: f64,
    /// Super 6: Banker wins with a total of 6.
    pub super_six: f64,
    pub rebate: Rebate,
}

impl Default for Payouts {
    fn default() -> Self {
        // Big/Small as dealt by Playtech; Super Tie as at the Bicycle Casino; Super 6 at the
        // common 12:1.
        Self {
            banker_commission: 0.05,
            tie: 8.0,
            big: 0.54,
            small: 1.5,
            super_tie: [100.0, 25.0, 7.0],
            pair: 11.0,
            super_six: 12.0,
            rebate: Rebate::NONE,
        }
    }
}

impl Payouts {
    /// Checks that every payout is 0 or more and every rate (commission, rebate) is from 0 to
    /// under 1.
    pub fn validate(&self) -> Result<(), String> {
        let is_rate = |rate: f64| (0.0..1.0).contains(&rate);
        let is_payout = |payout: f64| payout.is_finite() && payout >= 0.0;
        if !is_rate(self.banker_commission) {
            return Err(format!("banker_commission must be from 0 to under 1, found {}", self.banker_commission));
        }
        let payouts = [("tie", self.tie), ("big", self.big), ("small", self.small), ("pair", self.pair), ("super_six", self.super_six)];
        for (name, payout) in payouts {
            if !is_payout(payout) {
                return Err(format!("{name} must pay 0 or more, found {payout}"));
            }
        }
        if !self.super_tie.into_iter().all(is_payout) {
            return Err(format!("super_tie payouts must be 0 or more, found {:?}", self.super_tie));
        }
        for (bet, rate) in Bet::ALL.into_iter().zip(self.rebate.rates) {
            if !is_rate(rate) {
                return Err(format!("the rebate rate for {} must be from 0 to under 1, found {rate}", bet.key()));
            }
        }
        Ok(())
    }

    /// Net units won (negative when lost) by a 1-unit `bet` on a coup that ended in `result`,
    /// rebate included.
    pub fn net(&self, bet: Bet, result: &CoupResult) -> f64 {
        let before_rebate = self.net_before_rebate(bet, result);
        before_rebate + self.rebate(bet, before_rebate)
    }

    /// Rebate paid on a 1-unit `bet` that settled for `net_before_rebate` units.
    pub fn rebate(&self, bet: Bet, net_before_rebate: f64) -> f64 {
        let earns_rebate = match self.rebate.basis {
            RebateBasis::AllBets => true,
            RebateBasis::Losses => net_before_rebate < 0.0,
            RebateBasis::DecidedBets => net_before_rebate != 0.0,
        };
        if earns_rebate { self.rebate.rates[bet as usize] } else { 0.0 }
    }

    /// Net units won (negative when lost) by a 1-unit `bet` on a coup that ended in `result`,
    /// before any rebate.
    pub fn net_before_rebate(&self, bet: Bet, result: &CoupResult) -> f64 {
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
            Bet::PlayerPair => pays_if(result.player_pair, self.pair),
            Bet::BankerPair => pays_if(result.banker_pair, self.pair),
            Bet::SuperSix => pays_if(result.banker_six, self.super_six),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(outcome: Outcome, cards: usize, super_tie: Option<SuperTie>) -> CoupResult {
        CoupResult { outcome, cards, super_tie, banker_six: false, player_pair: false, banker_pair: false }
    }

    #[test]
    fn settles_every_bet() {
        let payouts = Payouts::default();
        let nets = |result: CoupResult| Bet::ALL.map(|bet| payouts.net(bet, &result));

        let banker_natural = result(Outcome::Banker, 4, None);
        assert_eq!(nets(banker_natural), [0.95, -1.0, -1.0, -1.0, 1.5, -1.0, -1.0, -1.0, -1.0]);

        let player_five_cards = result(Outcome::Player, 5, None);
        assert_eq!(nets(player_five_cards), [-1.0, 1.0, -1.0, 0.54, -1.0, -1.0, -1.0, -1.0, -1.0]);

        let color_tie = result(Outcome::Tie, 6, Some(SuperTie::SameColorSixCard));
        assert_eq!(nets(color_tie), [0.0, 0.0, 8.0, 0.54, -1.0, 100.0, -1.0, -1.0, -1.0]);

        let mixed_tie = CoupResult { super_tie: Some(SuperTie::MixedColorSixCard), ..color_tie };
        assert_eq!(payouts.net(Bet::SuperTie, &mixed_tie), 25.0);

        let four_card_tie = result(Outcome::Tie, 4, Some(SuperTie::OtherTie));
        assert_eq!(nets(four_card_tie), [0.0, 0.0, 8.0, -1.0, 1.5, 7.0, -1.0, -1.0, -1.0]);

        let banker_six_with_pairs = CoupResult { banker_six: true, player_pair: true, banker_pair: true, ..result(Outcome::Banker, 5, None) };
        assert_eq!(nets(banker_six_with_pairs), [0.95, -1.0, -1.0, 0.54, -1.0, -1.0, 11.0, 11.0, 12.0]);
    }

    #[test]
    fn validates_payouts_and_rates() {
        assert_eq!(Payouts::default().validate(), Ok(()));
        let bad = [
            Payouts { banker_commission: 1.0, ..Payouts::default() },
            Payouts { tie: -1.0, ..Payouts::default() },
            Payouts { super_tie: [100.0, f64::NAN, 7.0], ..Payouts::default() },
            Payouts { rebate: Rebate { rates: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.5], basis: RebateBasis::AllBets }, ..Payouts::default() },
        ];
        for payouts in bad {
            assert!(payouts.validate().is_err(), "{payouts:?}");
        }
    }

    #[test]
    fn pays_rebate_on_the_chosen_bets() {
        let tie = result(Outcome::Tie, 4, Some(SuperTie::OtherTie));
        let rebate_on = |basis| {
            let rates = [0.01, 0.01, 0.0, 0.02, 0.02, 0.0, 0.0, 0.0, 0.0];
            let payouts = Payouts { rebate: Rebate { rates, basis }, ..Payouts::default() };
            Bet::ALL.map(|bet| payouts.net(bet, &tie))
        };
        // On a 4-card tie: Banker and Player push, Tie wins, Big loses, Small wins, Super Tie wins.
        assert_eq!(rebate_on(RebateBasis::AllBets), [0.01, 0.01, 8.0, -0.98, 1.52, 7.0, -1.0, -1.0, -1.0]);
        assert_eq!(rebate_on(RebateBasis::Losses), [0.0, 0.0, 8.0, -0.98, 1.5, 7.0, -1.0, -1.0, -1.0]);
        assert_eq!(rebate_on(RebateBasis::DecidedBets), [0.0, 0.0, 8.0, -0.98, 1.52, 7.0, -1.0, -1.0, -1.0]);
    }
}
