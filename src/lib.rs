//! Baccarat (Punto Banco) odds: exact win rates and expected values of the Banker, Player,
//! Tie, Big, Small and Super Tie bets, plus a simulator that deals a full shoe.

pub mod analysis;
pub mod bets;
pub mod rules;
pub mod simulator;

pub use analysis::{BetStats, Odds, Shoe, analyze, card_points};
pub use bets::{Bet, Payouts, Rebate, RebateBasis};
pub use rules::{Card, Coup, CoupResult, Outcome, Suit, SuperTie};
pub use simulator::{DEFAULT_CUT, MIN_CUT, ShoeRun, simulate_shoe};
