//! Punto Banco rules: card values, the third-card rules and how a coup is dealt.

use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Card {
    /// 1 = Ace, 2-10, 11 = Jack, 12 = Queen, 13 = King.
    pub rank: u8,
    pub suit: Suit,
}

impl Card {
    /// Baccarat point value: 10/J/Q/K count 0, Ace 1, the rest face value.
    pub fn points(self) -> usize {
        if self.rank >= 10 { 0 } else { usize::from(self.rank) }
    }

    pub fn is_red(self) -> bool {
        matches!(self.suit, Suit::Hearts | Suit::Diamonds)
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suit = match self.suit {
            Suit::Spades => 's',
            Suit::Hearts => 'h',
            Suit::Diamonds => 'd',
            Suit::Clubs => 'c',
        };
        match self.rank {
            1 => write!(f, "A{suit}"),
            11 => write!(f, "J{suit}"),
            12 => write!(f, "Q{suit}"),
            13 => write!(f, "K{suit}"),
            rank => write!(f, "{rank}{suit}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Banker,
    Player,
    Tie,
}

/// Which Super Tie payout a tie earns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuperTie {
    /// Both hands drew a third card and tied on 6-9, and all six cards are one color.
    SameColorSixCard,
    /// Both hands drew a third card and tied on 6-9, with red and black cards mixed.
    MixedColorSixCard,
    /// Any other tie.
    OtherTie,
}

/// Everything about a finished coup that bets are settled on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoupResult {
    pub outcome: Outcome,
    /// Cards dealt to both hands together: 4, 5 or 6.
    pub cards: usize,
    /// `Some` exactly when the coup is a tie.
    pub super_tie: Option<SuperTie>,
    /// Banker won with a total of 6.
    pub banker_six: bool,
    /// The hand's first two cards are the same rank (K-K is a pair, K-Q is not).
    pub player_pair: bool,
    pub banker_pair: bool,
}

pub fn player_draws(player_total: usize) -> bool {
    player_total <= 5
}

/// Banker third-card rule; `player_third` is `None` when the player stood.
pub fn banker_draws(banker_total: usize, player_third: Option<usize>) -> bool {
    let Some(player_third) = player_third else {
        return banker_total <= 5;
    };
    match banker_total {
        0..=2 => true,
        3 => player_third != 8,
        4 => (2..=7).contains(&player_third),
        5 => (4..=7).contains(&player_third),
        6 => (6..=7).contains(&player_third),
        _ => false,
    }
}

pub fn winner(player_total: usize, banker_total: usize) -> Outcome {
    match banker_total.cmp(&player_total) {
        Ordering::Greater => Outcome::Banker,
        Ordering::Less => Outcome::Player,
        Ordering::Equal => Outcome::Tie,
    }
}

/// Whether the Super Tie bet pays a tie on `total` by color: both hands drew a third card
/// and tied on 6-9.
pub fn super_tie_by_color(cards: usize, total: usize) -> bool {
    cards == 6 && total >= 6
}

/// One coup as dealt: each hand's cards in the order they were drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coup {
    pub player: Vec<Card>,
    pub banker: Vec<Card>,
}

impl Coup {
    /// Deals a coup from the front of `cards`: P, B, P, B, then any third cards.
    ///
    /// Panics if `cards` runs out first; 6 cards always finish a coup.
    pub fn deal(cards: &[Card]) -> Self {
        let mut next_card = cards.iter().copied();
        let mut draw = || next_card.next().expect("not enough cards to finish the coup");
        let (p1, b1, p2, b2) = (draw(), draw(), draw(), draw());
        let mut coup = Coup { player: vec![p1, p2], banker: vec![b1, b2] };
        let (player, banker) = (coup.player_total(), coup.banker_total());
        if player < 8 && banker < 8 {
            let mut player_third = None;
            if player_draws(player) {
                let card = draw();
                coup.player.push(card);
                player_third = Some(card.points());
            }
            if banker_draws(banker, player_third) {
                coup.banker.push(draw());
            }
        }
        coup
    }

    pub fn player_total(&self) -> usize {
        hand_total(&self.player)
    }

    pub fn banker_total(&self) -> usize {
        hand_total(&self.banker)
    }

    pub fn cards_used(&self) -> usize {
        self.player.len() + self.banker.len()
    }

    pub fn result(&self) -> CoupResult {
        let total = self.player_total();
        let banker_total = self.banker_total();
        let outcome = winner(total, banker_total);
        let cards = self.cards_used();
        let super_tie = (outcome == Outcome::Tie).then(|| {
            let first_is_red = self.player[0].is_red();
            if !super_tie_by_color(cards, total) {
                SuperTie::OtherTie
            } else if self.player.iter().chain(&self.banker).all(|card| card.is_red() == first_is_red) {
                SuperTie::SameColorSixCard
            } else {
                SuperTie::MixedColorSixCard
            }
        });
        CoupResult {
            outcome,
            cards,
            super_tie,
            banker_six: outcome == Outcome::Banker && banker_total == 6,
            player_pair: self.player[0].rank == self.player[1].rank,
            banker_pair: self.banker[0].rank == self.banker[1].rank,
        }
    }
}

fn hand_total(cards: &[Card]) -> usize {
    cards.iter().map(|card| card.points()).sum::<usize>() % 10
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(rank: u8, suit: Suit) -> Card {
        Card { rank, suit }
    }

    #[test]
    fn banker_rule_matches_third_card_table() {
        // banker total -> player third cards on which the banker draws
        let draws_on: [(usize, &[usize]); 8] = [
            (0, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            (1, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            (2, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            (3, &[0, 1, 2, 3, 4, 5, 6, 7, 9]),
            (4, &[2, 3, 4, 5, 6, 7]),
            (5, &[4, 5, 6, 7]),
            (6, &[6, 7]),
            (7, &[]),
        ];
        for (banker_total, expected) in draws_on {
            let actual: Vec<usize> = (0..10).filter(|&card| banker_draws(banker_total, Some(card))).collect();
            assert_eq!(actual, expected, "banker total {banker_total}");
        }
    }

    #[test]
    fn banker_plays_like_player_when_player_stood() {
        let draws: Vec<usize> = (0..8).filter(|&total| banker_draws(total, None)).collect();
        assert_eq!(draws, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn natural_ends_the_coup_after_four_cards() {
        let cards = [8, 1, 13, 2, 5, 5].map(|rank| card(rank, Suit::Clubs));
        let coup = Coup::deal(&cards);
        let expected = CoupResult { outcome: Outcome::Player, cards: 4, super_tie: None, banker_six: false, player_pair: false, banker_pair: false };
        assert_eq!(coup.result(), expected);
    }

    #[test]
    fn finds_pairs_by_rank_and_banker_wins_with_six() {
        // Player K + K = 0 draws a 5; Banker 3 + 3 = 6 stands on the Player's 5 and wins with 6.
        let cards = [card(13, Suit::Clubs), card(3, Suit::Hearts), card(13, Suit::Spades), card(3, Suit::Clubs), card(5, Suit::Clubs)];
        let result = Coup::deal(&cards).result();
        assert!(result.player_pair && result.banker_pair && result.banker_six);

        // K and Q are both worth 0 but are not a pair.
        let mut mixed_faces = cards;
        mixed_faces[2].rank = 12;
        assert!(!Coup::deal(&mixed_faces).result().player_pair);
    }

    #[test]
    fn six_card_tie_on_6_to_9_is_paid_by_color() {
        // Player 3 + 2 = 5 draws a 2 for 7; Banker 4 + K = 4 draws on that 2 and gets a 3 for 7.
        let red = [card(3, Suit::Hearts), card(4, Suit::Hearts), card(2, Suit::Diamonds), card(13, Suit::Hearts), card(2, Suit::Hearts), card(3, Suit::Diamonds)];
        let result = Coup::deal(&red).result();
        assert_eq!((result.outcome, result.cards, result.super_tie), (Outcome::Tie, 6, Some(SuperTie::SameColorSixCard)));

        let mut mixed = red;
        mixed[5].suit = Suit::Clubs;
        assert_eq!(Coup::deal(&mixed).result().super_tie, Some(SuperTie::MixedColorSixCard));
    }
}
