import itertools
import math
import unittest
from collections import Counter
from fractions import Fraction

from baccarat_ev import BANKER, PLAYER, TIE, analyze, banker_draws, full_shoe, player_draws, remove_cards


def play_coup(cards):
    """Deal one coup from an ordered sequence of card values, the way a dealer does."""
    deck = iter(cards)
    p1, b1, p2, b2 = next(deck), next(deck), next(deck), next(deck)
    player, banker = (p1 + p2) % 10, (b1 + b2) % 10
    if player < 8 and banker < 8:
        player_third = None
        if player_draws(player):
            player_third = next(deck)
            player = (player + player_third) % 10
        if banker_draws(banker, player_third):
            banker = (banker + next(deck)) % 10
    if banker > player:
        return BANKER
    return PLAYER if player > banker else TIE


class BankerDrawRuleTest(unittest.TestCase):
    def test_matches_third_card_table(self):
        draws_on = {  # banker total -> player third cards on which banker draws
            0: set(range(10)), 1: set(range(10)), 2: set(range(10)),
            3: set(range(10)) - {8}, 4: {2, 3, 4, 5, 6, 7}, 5: {4, 5, 6, 7}, 6: {6, 7}, 7: set(),
        }
        for banker_total, expected in draws_on.items():
            actual = {card for card in range(10) if banker_draws(banker_total, card)}
            self.assertEqual(actual, expected, f"banker total {banker_total}")

    def test_player_stood(self):
        self.assertEqual([total for total in range(8) if banker_draws(total, None)], [0, 1, 2, 3, 4, 5])


class AnalyzeTest(unittest.TestCase):
    def test_matches_every_ordering_of_a_small_shoe(self):
        for cards in [(0, 0, 1, 2, 3, 4, 5, 6), (0, 3, 4, 5, 6, 7, 8, 9), (0, 0, 0, 1, 1, 7, 8, 9)]:
            outcomes = Counter(play_coup(order) for order in itertools.permutations(cards))
            orderings = math.factorial(len(cards))
            shoe = [cards.count(value) for value in range(10)]
            odds = analyze(shoe)
            self.assertEqual(odds.banker, Fraction(outcomes[BANKER], orderings), cards)
            self.assertEqual(odds.player, Fraction(outcomes[PLAYER], orderings), cards)
            self.assertEqual(odds.tie, Fraction(outcomes[TIE], orderings), cards)

    def test_eight_deck_shoe_matches_published_values(self):
        odds = analyze(full_shoe(8))
        self.assertEqual(odds.banker + odds.player + odds.tie, 1)
        self.assertAlmostEqual(float(odds.banker), 0.458597, places=6)
        self.assertAlmostEqual(float(odds.player), 0.446247, places=6)
        self.assertAlmostEqual(float(odds.tie), 0.095156, places=6)
        self.assertAlmostEqual(float(odds.banker_ev()), -0.010579, places=6)
        self.assertAlmostEqual(float(odds.player_ev()), -0.012351, places=6)
        self.assertAlmostEqual(float(odds.tie_ev()), -0.143596, places=6)

    def test_rejects_shoe_too_small_for_a_coup(self):
        with self.assertRaises(ValueError):
            analyze([5, 0, 0, 0, 0, 0, 0, 0, 0, 0])


class RemoveCardsTest(unittest.TestCase):
    def test_maps_ranks_to_points(self):
        shoe = remove_cards(full_shoe(1), ["K", "10", "a", "9"])
        self.assertEqual(shoe, [14, 3, 4, 4, 4, 4, 4, 4, 4, 3])

    def test_rejects_card_not_in_shoe(self):
        with self.assertRaises(ValueError):
            remove_cards([0, 0, 0, 0, 0, 0, 0, 0, 0, 1], ["5"])


if __name__ == "__main__":
    unittest.main()
