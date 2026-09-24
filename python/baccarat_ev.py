"""Exact win rates and expected values for the Banker, Player and Tie bets in
Punto Banco baccarat.

A shoe is a list of 10 counts: shoe[v] is the number of cards worth v points
(0 = 10/J/Q/K, 1 = Ace, 2-9 = face value). The algorithm walks every possible
coup (P1, B1, P2, B2, optional P3, optional B3) and weights each one by the
number of ordered ways to draw those cards without replacement, so the result
is exact for any composition: a fresh shoe or one that is partly dealt.
"""

from __future__ import annotations

import argparse
from collections.abc import Iterator
from dataclasses import dataclass
from fractions import Fraction

CARDS_PER_DECK = (16, 4, 4, 4, 4, 4, 4, 4, 4, 4)
CARD_POINTS = {"A": 1, "T": 0, "10": 0, "J": 0, "Q": 0, "K": 0, **{str(value): value for value in range(10)}}
BANKER, PLAYER, TIE = 0, 1, 2


def full_shoe(decks: int) -> list[int]:
    """Card counts by point value for a freshly shuffled shoe."""
    return [count * decks for count in CARDS_PER_DECK]


def remove_cards(shoe: list[int], cards: list[str]) -> list[int]:
    """Return the shoe left after dealing `cards` (ranks like "K", "A", "7" or "10")."""
    shoe = list(shoe)
    for card in cards:
        value = CARD_POINTS.get(card.upper())
        if value is None:
            raise ValueError(f"unknown card {card!r}")
        if shoe[value] == 0:
            raise ValueError(f"no card worth {value} left to remove for {card!r}")
        shoe[value] -= 1
    return shoe


def player_draws(player_total: int) -> bool:
    return player_total <= 5


def banker_draws(banker_total: int, player_third: int | None) -> bool:
    """Banker third-card rule; `player_third` is None when the player stood."""
    if player_third is None:
        return banker_total <= 5
    if banker_total <= 2:
        return True
    if banker_total == 3:
        return player_third != 8
    if banker_total == 4:
        return 2 <= player_third <= 7
    if banker_total == 5:
        return 4 <= player_third <= 7
    if banker_total == 6:
        return 6 <= player_third <= 7
    return False


def _winner(player_total: int, banker_total: int) -> int:
    if banker_total > player_total:
        return BANKER
    if player_total > banker_total:
        return PLAYER
    return TIE


def _draw(shoe: list[int]) -> Iterator[tuple[int, int]]:
    """Yield (value, ways) for every card value still in the shoe.

    The card stays out of the shoe while the caller's loop body runs, so nested
    loops see the shoe exactly as it is at that point of the deal.
    """
    for value, ways in enumerate(shoe):
        if ways:
            shoe[value] -= 1
            try:
                yield value, ways
            finally:
                shoe[value] += 1


def _initial_deals(shoe: list[int]) -> Iterator[tuple[int, int, int]]:
    """Yield (player_total, banker_total, ways) for every ordered P1-B1-P2-B2 deal."""
    for p1, ways1 in _draw(shoe):
        for b1, ways2 in _draw(shoe):
            for p2, ways3 in _draw(shoe):
                for b2, ways4 in _draw(shoe):
                    yield (p1 + p2) % 10, (b1 + b2) % 10, ways1 * ways2 * ways3 * ways4


@dataclass(frozen=True)
class Odds:
    """Probability of each hand outcome for one coup."""

    banker: Fraction
    player: Fraction
    tie: Fraction

    def banker_ev(self, commission: Fraction = Fraction(5, 100)) -> Fraction:
        """Expected net units per unit bet on Banker (wins pay 1:1 less commission, ties push)."""
        return self.banker * (1 - commission) - self.player

    def player_ev(self) -> Fraction:
        """Expected net units per unit bet on Player (wins pay 1:1, ties push)."""
        return self.player - self.banker

    def tie_ev(self, payout: int = 8) -> Fraction:
        """Expected net units per unit bet on Tie (wins pay `payout`:1, otherwise lose)."""
        return self.tie * payout - (1 - self.tie)


def analyze(shoe: list[int]) -> Odds:
    """Exact outcome probabilities of the next coup dealt from `shoe`."""
    if len(shoe) != 10 or min(shoe) < 0:
        raise ValueError("shoe must hold 10 non-negative counts, one per point value 0-9")
    cards = sum(shoe)
    if cards < 6:
        raise ValueError("a coup can use 6 cards, so the shoe must hold at least 6")

    shoe = list(shoe)
    # A coup that uses k cards is also weighted by the ordered ways to draw the
    # 6 - k cards it leaves unused, so every coup shares the denominator N(N-1)...(N-5).
    unused_after_4 = (cards - 4) * (cards - 5)
    unused_after_5 = cards - 5
    tally = [0, 0, 0]
    for player, banker, ways in _initial_deals(shoe):
        if player >= 8 or banker >= 8:  # natural: both hands stand
            tally[_winner(player, banker)] += ways * unused_after_4
        elif player_draws(player):
            for player_third, third_ways in _draw(shoe):
                final_player = (player + player_third) % 10
                if banker_draws(banker, player_third):
                    for banker_third, last_ways in _draw(shoe):
                        tally[_winner(final_player, (banker + banker_third) % 10)] += ways * third_ways * last_ways
                else:
                    tally[_winner(final_player, banker)] += ways * third_ways * unused_after_5
        elif banker_draws(banker, None):
            for banker_third, third_ways in _draw(shoe):
                tally[_winner(player, (banker + banker_third) % 10)] += ways * third_ways * unused_after_5
        else:
            tally[_winner(player, banker)] += ways * unused_after_4

    total = cards * (cards - 1) * (cards - 2) * (cards - 3) * (cards - 4) * (cards - 5)
    return Odds(*(Fraction(count, total) for count in tally))


def format_report(odds: Odds, commission: Fraction = Fraction(5, 100), tie_payout: int = 8) -> str:
    decided = odds.banker + odds.player
    rows = [
        ("Banker", odds.banker, odds.player, odds.tie, odds.banker_ev(commission), odds.banker / decided if decided else None),
        ("Player", odds.player, odds.banker, odds.tie, odds.player_ev(), odds.player / decided if decided else None),
        ("Tie", odds.tie, 1 - odds.tie, Fraction(0), odds.tie_ev(tie_payout), None),
    ]
    lines = [f"{'Bet':<8}{'Win':>10}{'Lose':>10}{'Push':>10}{'EV / unit':>12}{'Win rate excl. ties':>22}"]
    for name, win, lose, push, ev, decided_rate in rows:
        decided_text = f"{float(decided_rate):.4%}" if decided_rate is not None else "-"
        lines.append(f"{name:<8}{float(win):>10.4%}{float(lose):>10.4%}{float(push):>10.4%}{float(ev):>12.4%}{decided_text:>22}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Exact Banker/Player/Tie win rates and EV for baccarat (Punto Banco).")
    parser.add_argument("--decks", type=int, default=8, help="decks in the shoe (default: 8)")
    parser.add_argument("--dealt", nargs="*", default=[], metavar="CARD",
                        help="cards already dealt from the shoe, e.g. --dealt K 5 A 9 10")
    parser.add_argument("--commission", type=Fraction, default=Fraction(5, 100), help="Banker commission (default: 0.05)")
    parser.add_argument("--tie-pays", type=int, default=8, help="Tie payout N for N:1 (default: 8)")
    args = parser.parse_args(argv)

    shoe = remove_cards(full_shoe(args.decks), args.dealt)
    odds = analyze(shoe)
    print(f"Shoe: {sum(shoe)} cards left ({args.decks} decks, {len(args.dealt)} dealt)\n")
    print(format_report(odds, args.commission, args.tie_pays))


if __name__ == "__main__":
    main()
