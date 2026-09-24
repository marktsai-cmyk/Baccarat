//! Deals a full shoe the way a Punto Banco table does: shuffle, burn, then deal to the cut card.

use crate::rules::{Card, Coup, Suit};

/// Cut card position, in cards from the end of the shoe, when none is given.
pub const DEFAULT_CUT: usize = 14;

/// The coup in progress can run 5 cards past the cut card and the one dealt after it needs
/// up to 6 more, so the cut card must sit at least 11 cards from the end.
pub const MIN_CUT: usize = 11;

/// One shoe from shuffle to cut card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShoeRun {
    /// The exposed first card followed by the cards burned because of it.
    pub burned: Vec<Card>,
    pub coups: Vec<Coup>,
    /// Cards still in the shoe after the last coup.
    pub cards_left: usize,
}

/// Shuffles `decks` decks with `seed`, then deals coups until the cut card, `cut` cards from
/// the end, comes out: the coup in progress is finished and one more is dealt.
pub fn simulate_shoe(decks: u32, cut: usize, seed: u64) -> Result<ShoeRun, String> {
    if decks == 0 {
        return Err("the shoe needs at least one deck".to_string());
    }
    let mut cards = new_shoe(decks);
    if !(MIN_CUT..=cards.len()).contains(&cut) {
        return Err(format!("the cut card must be {MIN_CUT} to {} cards from the end", cards.len()));
    }
    shuffle(&mut cards, &mut Rng::new(seed));

    // The first card is exposed and that many more are burned, with 10/J/Q/K burning 10.
    let burned = 1 + usize::from(cards[0].rank.min(10));
    let cut_index = cards.len() - cut;
    let mut next = burned;
    let mut coups = Vec::new();
    loop {
        let cut_card_is_out = next >= cut_index;
        let coup = Coup::deal(&cards[next..]);
        next += coup.cards_used();
        coups.push(coup);
        if cut_card_is_out {
            break;
        }
    }
    Ok(ShoeRun { burned: cards[..burned].to_vec(), coups, cards_left: cards.len() - next })
}

fn new_shoe(decks: u32) -> Vec<Card> {
    let mut cards = Vec::new();
    for _ in 0..decks {
        for suit in Suit::ALL {
            for rank in 1..=13 {
                cards.push(Card { rank, suit });
            }
        }
    }
    cards
}

/// Fisher-Yates shuffle.
fn shuffle(cards: &mut [Card], rng: &mut Rng) {
    for i in (1..cards.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        cards.swap(i, j);
    }
}

/// xoshiro256** random number generator, seeded through SplitMix64.
struct Rng {
    state: [u64; 4],
}

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut split_mix = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Self { state: [split_mix(), split_mix(), split_mix(), split_mix()] }
    }

    fn next_u64(&mut self) -> u64 {
        let s = &mut self.state;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform integer in `0..bound`, without modulo bias (Lemire's multiply-and-reject).
    fn below(&mut self, bound: u64) -> u64 {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let product = u128::from(self.next_u64()) * u128::from(bound);
            if product as u64 >= threshold {
                return (product >> 64) as u64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_keeps_every_card() {
        let mut cards = new_shoe(2);
        shuffle(&mut cards, &mut Rng::new(7));
        assert_ne!(cards, new_shoe(2));
        cards.sort();
        let mut expected = new_shoe(2);
        expected.sort();
        assert_eq!(cards, expected);
    }

    #[test]
    fn burns_the_first_card_and_that_many_more() {
        for seed in 0..100 {
            let run = simulate_shoe(8, DEFAULT_CUT, seed).unwrap();
            assert_eq!(run.burned.len(), 1 + usize::from(run.burned[0].rank.min(10)), "seed {seed}");
        }
    }

    #[test]
    fn deals_one_more_coup_after_the_cut_card_comes_out() {
        for cut in [MIN_CUT, DEFAULT_CUT, 100] {
            for seed in 0..200 {
                let run = simulate_shoe(8, cut, seed).unwrap();
                let cut_index = 8 * 52 - cut;
                let mut next = run.burned.len();
                for (number, coup) in run.coups.iter().enumerate() {
                    let is_last = number == run.coups.len() - 1;
                    assert_eq!(next >= cut_index, is_last, "cut {cut}, seed {seed}, coup {number}");
                    next += coup.cards_used();
                }
                assert_eq!(next + run.cards_left, 8 * 52);
            }
        }
    }

    #[test]
    fn same_seed_replays_the_same_shoe() {
        assert_eq!(simulate_shoe(8, DEFAULT_CUT, 42), simulate_shoe(8, DEFAULT_CUT, 42));
        assert_ne!(simulate_shoe(8, DEFAULT_CUT, 42), simulate_shoe(8, DEFAULT_CUT, 43));
    }

    #[test]
    fn rejects_cut_card_it_cannot_honor() {
        assert!(simulate_shoe(8, MIN_CUT - 1, 1).is_err());
        assert!(simulate_shoe(1, 53, 1).is_err());
        assert!(simulate_shoe(0, DEFAULT_CUT, 1).is_err());
    }
}
