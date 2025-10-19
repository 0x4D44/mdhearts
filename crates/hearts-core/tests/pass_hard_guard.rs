use hearts_core::game::match_state::MatchState;
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::{PassingDirection, PassingState};
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use hearts_core::moon::{MoonEstimate, MoonObjective};
use hearts_core::pass::direction::DirectionProfile;
use hearts_core::pass::optimizer::{enumerate_pass_triples, force_guarded_pass};
use hearts_core::pass::scoring::{PassScoreInput, PassWeights};
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn base_seed_for(hand_index: usize) -> u64 {
    let mut rng = StdRng::seed_from_u64(20251017);
    let mut base_seed = 0u64;
    for _ in 0..=hand_index {
        base_seed = rng.next_u64();
    }
    base_seed
}

fn make_input(cards: Vec<Card>, seat: PlayerPosition) -> PassScoreInput<'static> {
    let passing = PassingDirection::Left;
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[seat.index()] = Hand::with_cards(cards);

    let round = RoundState::from_hands(
        hands,
        seat,
        passing,
        RoundPhase::Passing(PassingState::new(passing)),
    );
    let round = Box::leak(Box::new(round));
    let scores = Box::leak(Box::new(ScoreBoard::new()));
    let moon_estimate = MoonEstimate {
        probability: 0.7,
        raw_score: 1.3,
        objective: MoonObjective::BlockShooter,
    };

    PassScoreInput {
        seat,
        hand: round.hand(seat),
        round,
        scores,
        belief: None,
        weights: PassWeights::default(),
        direction: passing,
        direction_profile: DirectionProfile::from_direction(passing),
        moon_estimate,
    }
}

fn candidate_contains(cards: &[Card; 3], target: &[Card]) -> bool {
    target.iter().all(|card| cards.contains(card))
}

fn is_high_support(card: &Card) -> bool {
    card.suit == Suit::Hearts && card.rank >= Rank::Ten && card.rank < Rank::Queen
}

fn is_mid_support(card: &Card) -> bool {
    card.suit == Suit::Hearts && card.rank >= Rank::Eight && card.rank < Rank::Ten
}

fn support_total(cards: &[Card; 3]) -> usize {
    cards.iter().filter(|card| is_high_support(card)).count()
        + cards.iter().filter(|card| is_mid_support(card)).count()
}

fn stage2_input(hand_index: usize, seat: PlayerPosition) -> PassScoreInput<'static> {
    let base_seed = base_seed_for(hand_index);
    let state = MatchState::with_seed(PlayerPosition::North, base_seed);
    let round = state.round();
    let cards: Vec<Card> = round.hand(seat).iter().copied().collect();
    assert_eq!(
        cards.len(),
        13,
        "expected full hand for hand {hand_index} seat {seat:?}"
    );
    make_input(cards, seat)
}

#[test]
fn stage2_force_guarded_pass_preserves_stoppers() {
    struct Case {
        hand_index: usize,
        seat: PlayerPosition,
        forbid_ace: bool,
        forbid_king: bool,
    }
    let cases = [
        Case {
            hand_index: 75,
            seat: PlayerPosition::West,
            forbid_ace: true,
            forbid_king: false,
        },
        Case {
            hand_index: 511,
            seat: PlayerPosition::South,
            forbid_ace: true,
            forbid_king: true,
        },
        Case {
            hand_index: 757,
            seat: PlayerPosition::North,
            forbid_ace: true,
            forbid_king: true,
        },
        Case {
            hand_index: 767,
            seat: PlayerPosition::North,
            forbid_ace: true,
            forbid_king: true,
        },
    ];

    for case in cases {
        let input = stage2_input(case.hand_index, case.seat);
        if let Some(forced) = force_guarded_pass(&input) {
            if case.forbid_ace {
                assert!(
                    !forced
                        .cards
                        .iter()
                        .any(|card| { card.suit == Suit::Hearts && card.rank == Rank::Ace }),
                    "forced pass should retain A♥ for hand {} seat {:?}, got {:?}",
                    case.hand_index,
                    case.seat,
                    forced.cards
                );
            }
            if case.forbid_king {
                assert!(
                    !forced
                        .cards
                        .iter()
                        .any(|card| { card.suit == Suit::Hearts && card.rank == Rank::King }),
                    "forced pass should retain K♥ for hand {} seat {:?}, got {:?}",
                    case.hand_index,
                    case.seat,
                    forced.cards
                );
            }
        } else {
            let combos = enumerate_pass_triples(&input);
            assert!(
                combos.iter().all(|cand| {
                    (!case.forbid_ace
                        || !cand
                            .cards
                            .iter()
                            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Ace))
                        && (!case.forbid_king
                            || !cand
                                .cards
                                .iter()
                                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::King))
                }),
                "enumerator should preserve stoppers for hand {} seat {:?}",
                case.hand_index,
                case.seat
            );
        }
    }
}

#[test]
fn stage2_hand_153_keeps_ace_after_guard() {
    let input = stage2_input(153, PlayerPosition::South);
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.iter().all(|candidate| {
            !candidate
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Ace)
        }),
        "expected enumerator to retain A♥, sample={:?}",
        combos
            .iter()
            .take(5)
            .map(|c| c.cards)
            .collect::<Vec<[Card; 3]>>()
    );
}

#[test]
fn stage2_hand_582_rejects_premium_without_support() {
    let input = stage2_input(582, PlayerPosition::East);
    let combos = enumerate_pass_triples(&input);
    if combos.is_empty() {
        if let Some(forced) = force_guarded_pass(&input) {
            assert!(
                forced.cards.iter().any(|card| card.suit == Suit::Hearts),
                "forced pass should retain a heart: {:?}",
                forced.cards
            );
            let has_queen = forced
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen);
            if has_queen {
                let support = support_total(&forced.cards);
                if support < 2 {
                    let remaining_support = input
                        .hand
                        .iter()
                        .filter(|card| {
                            card.suit == Suit::Hearts
                                && (is_high_support(card) || is_mid_support(card))
                                && !forced.cards.contains(card)
                        })
                        .count();
                    assert_eq!(
                        remaining_support, 0,
                        "unsupported Q♥ pass should only fire when no additional support hearts remain: {:?}",
                        forced.cards
                    );
                    assert!(
                        forced.cards.iter().any(|card| {
                            card.suit != Suit::Hearts
                                && (card.is_queen_of_spades() || card.rank >= Rank::King)
                        }),
                        "unsupported Q♥ pass should pair with a high off-suit liability: {:?}",
                        forced.cards
                    );
                }
            }
        }
    } else {
        for cand in combos.iter() {
            let passes_guard = cand.cards.iter().any(|card| {
                card.suit == Suit::Hearts && (card.rank == Rank::Queen || card.rank == Rank::Jack)
            });
            if passes_guard {
                let has_queen = cand
                    .cards
                    .iter()
                    .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen);
                let required = if has_queen { 2 } else { 1 };
                let support = support_total(&cand.cards);
                if support < required && has_queen {
                    let remaining_support = input
                        .hand
                        .iter()
                        .filter(|card| {
                            card.suit == Suit::Hearts
                                && (is_high_support(card) || is_mid_support(card))
                                && !cand.cards.contains(card)
                        })
                        .count();
                    assert_eq!(
                        remaining_support, 0,
                        "enumerator should only relax Q♥ support when no additional support hearts remain: {:?}",
                        cand.cards
                    );
                    assert!(
                        cand.cards.iter().any(|card| {
                            card.suit != Suit::Hearts
                                && (card.is_queen_of_spades() || card.rank >= Rank::King)
                        }),
                        "unsupported Q♥ candidate should pair with a high off-suit liability: {:?}",
                        cand.cards
                    );
                } else {
                    assert!(
                        support >= required,
                        "expected premium heart passes to include at least {required} support: {:?}",
                        cand.cards
                    );
                }
            }
        }
    }
}

#[test]
fn stage2_hand_26_rejects_soft_anchor_liability() {
    let input = stage2_input(26, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    if combos.is_empty() {
        let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 26");
        assert!(
            forced
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts)
                .count()
                >= 2,
            "forced path should ship double-heart coverage: {:?}",
            forced.cards
        );
        assert!(
            !forced
                .cards
                .iter()
                .any(|card| *card == Card::new(Rank::King, Suit::Clubs)),
            "forced path should not rely on soft K♣ anchor: {:?}",
            forced.cards
        );
    } else {
        for candidate in combos {
            assert!(
                candidate
                    .cards
                    .iter()
                    .filter(|card| card.suit == Suit::Hearts)
                    .count()
                    >= 2,
                "hand 26 should ship double-heart support, got {:?}",
                candidate.cards
            );
            assert!(
                !candidate
                    .cards
                    .iter()
                    .any(|card| *card == Card::new(Rank::King, Suit::Clubs)),
                "hand 26 should not rely on soft K♣ anchor: {:?}",
                candidate.cards
            );
        }
    }
}

#[test]
fn stage2_hand_212_blocks_queen_ten_mix() {
    let input = stage2_input(212, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.is_empty(),
        "hand 212 should reject Q♠ + J♥ mixes outright"
    );
    let forced = force_guarded_pass(&input).expect("forced guard should synthesize a pass");
    assert!(
        forced.cards.iter().all(|card| card.suit != Suit::Hearts),
        "hand 212 forced fallback should avoid shipping unsupported hearts: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Spades && card.rank >= Rank::Queen)
            .count()
            >= 1,
        "hand 212 forced fallback should anchor on a premium spade: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_599_passes_both_low_hearts() {
    let input = stage2_input(599, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.iter().all(|candidate| {
            candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts)
                .count()
                == 2
        }),
        "hand 599 should pass both low hearts for coverage"
    );
}

#[test]
fn stage2_hand_928_forces_strong_spade_anchor() {
    let input = stage2_input(928, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("forced guard should synthesize a pass");
    assert!(
        forced
            .cards
            .iter()
            .any(|card| *card == Card::new(Rank::Nine, Suit::Hearts)),
        "hand 928 forced pass should include the 9♥ anchor: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Spades && card.rank >= Rank::King)
            .count()
            >= 2,
        "hand 928 forced pass should pair heart support with double premium spades: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_420_retains_low_heart_control() {
    let input = stage2_input(420, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.is_empty(),
        "enumerator should refuse to ship the lone low heart"
    );
    let forced = force_guarded_pass(&input).expect("forced guard should synthesize a pass");
    assert!(
        forced.cards.iter().all(|card| card.suit != Suit::Hearts),
        "forced pass should keep the last heart at home: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .any(|card| { card.suit == Suit::Spades && card.rank >= Rank::King }),
        "forced pass should include a strong spade liability: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_96_pairs_queen_with_spade_anchor() {
    let input = stage2_input(96, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    for cand in combos {
        let passes_qspade = cand.cards.iter().any(|card| card.is_queen_of_spades());
        let passed_hearts: Vec<_> = cand
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts)
            .collect();
        if passes_qspade && passed_hearts.len() == 1 {
            assert!(
                cand.cards.iter().any(|card| {
                    card.suit == Suit::Spades
                        && card.rank >= Rank::King
                        && !card.is_queen_of_spades()
                }),
                "Q♠ passes must include an extra spade liability anchor: {:?}",
                cand.cards
            );
        }
    }
}

#[test]
fn stage2_hand_941_forced_builds_spade_void() {
    let input = stage2_input(941, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("forced guard should synthesize a pass");
    assert!(
        forced.cards.iter().all(|card| card.suit != Suit::Hearts),
        "forced pass should keep the lone low heart: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .any(|card| { card.suit == Suit::Spades && card.rank >= Rank::King }),
        "forced pass should include a spade liability anchor: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_594_forces_mid_support_for_queen() {
    let input = stage2_input(594, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("expected forced candidate");
    assert!(
        forced
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen),
        "forced combo should include Q♥: {:?}",
        forced.cards
    );
    assert!(
        support_total(&forced.cards) >= 2,
        "expected Q♥ pass to ship robust Ten+/mid support: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_912_requires_double_support_for_premium_hearts() {
    let input = stage2_input(912, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    if combos.is_empty() {
        return;
    }
    for cand in combos.iter() {
        let passes_guard = cand.cards.iter().any(|card| {
            card.suit == Suit::Hearts && (card.rank == Rank::Queen || card.rank == Rank::Jack)
        });
        if passes_guard {
            let has_queen = cand
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen);
            let required = if has_queen { 2 } else { 1 };
            assert!(
                support_total(&cand.cards) >= required,
                "expected premium heart passes to include at least {required} support: {:?}",
                cand.cards
            );
        }
    }
}

#[test]
fn stage2_hand_526_requires_liability_anchor_for_jack() {
    let input = stage2_input(526, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.is_empty(),
        "expected enumerator to return candidates for hand 526"
    );
    for cand in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Jack)
    }) {
        assert!(
            !cand
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen),
            "jack support should no longer ship Q♥ when only soft anchors exist: {:?}",
            cand.cards
        );
        let has_high_support = cand.cards.iter().any(|card| {
            card.suit == Suit::Hearts
                && card.rank >= Rank::Ten
                && card.rank < Rank::Queen
                && card.rank != Rank::Jack
        });
        let has_liability_anchor = cand.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.rank >= Rank::King || card.rank == Rank::Queen)
        });
        assert!(
            has_high_support || has_liability_anchor,
            "jack pass should include Ten+ support or an off-suit anchor: {:?}",
            cand.cards
        );
    }
    if let Some(forced) = force_guarded_pass(&input) {
        let has_anchor = forced.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.rank >= Rank::King || card.rank == Rank::Queen)
        });
        assert!(
            has_anchor,
            "forced fallback should pair J♥ with a high off-suit: {:?}",
            forced.cards
        );
    }
}

#[test]
fn stage2_hand_699_forces_jack_with_premium_anchor() {
    let input = stage2_input(699, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 699");
    assert!(
        forced
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Jack),
        "forced fallback should include J♥: {:?}",
        forced.cards
    );
    assert!(
        forced.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.rank >= Rank::King || card.is_queen_of_spades())
        }),
        "forced fallback should anchor J♥ with a premium off-suit: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_904_requires_king_anchor_for_queen() {
    let input = stage2_input(904, PlayerPosition::West);
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.is_empty(),
        "expected enumerator to return candidates for hand 904"
    );
    for cand in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen)
    }) {
        let has_anchor = cand.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.rank >= Rank::King || card.is_queen_of_spades())
        });
        assert!(
            has_anchor,
            "queen pass should include a premium off-suit anchor: {:?}",
            cand.cards
        );
    }
}

#[test]
fn stage2_hand_912_promotes_liability_for_premium_dump() {
    let input = stage2_input(912, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 912");
    assert!(
        forced.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.rank >= Rank::King || card.is_queen_of_spades())
        }),
        "forced fallback should pair premium hearts with an off-suit liability: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count()
            <= 2,
        "forced fallback should avoid dumping all Ten+ hearts: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_320_rejects_all_offsuit_candidates() {
    let input = stage2_input(320, PlayerPosition::East);
    let combos = enumerate_pass_triples(&input);
    if combos.is_empty() {
        let forced = force_guarded_pass(&input).expect("expected forced candidate");
        assert!(
            forced.cards.iter().any(|card| card.suit == Suit::Hearts),
            "forced pass should include a heart: {:?}",
            forced.cards
        );
    } else {
        for cand in combos {
            assert!(
                cand.cards.iter().any(|card| card.suit == Suit::Hearts),
                "expected at least one heart in candidate: {:?}",
                cand.cards
            );
        }
    }
}

#[test]
fn stage2_hand_44_requires_liability_anchor() {
    let input = stage2_input(44, PlayerPosition::South);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 44");
    assert!(
        forced.cards.iter().any(|card| card.suit != Suit::Hearts),
        "forced pass should include off-suit liability anchors: {:?}",
        forced.cards
    );
    assert!(
        !forced
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen),
        "queen should be retained when no legitimate support exists: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Jack),
        "forced pass should still ship the highest controllable heart (J♥) for pressure: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_380_forced_includes_liability_mix() {
    let input = stage2_input(380, PlayerPosition::South);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 380");
    assert!(
        forced.cards.iter().any(|card| card.suit != Suit::Hearts),
        "forced pass should include a liability off-suit card: {:?}",
        forced.cards
    );
    assert!(
        forced.cards.iter().any(|card| card.suit == Suit::Hearts),
        "forced pass should preserve heart coverage: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_64_demotes_triple_premium() {
    let input = stage2_input(64, PlayerPosition::West);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 64");
    let premium_count = forced
        .cards
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    assert!(
        premium_count <= 1,
        "forced pass should demote multiple premium hearts: {:?}",
        forced.cards
    );
    assert!(
        forced.cards.iter().any(|card| card.suit != Suit::Hearts),
        "forced pass should include off-suit liability: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_487_demotes_triple_premium() {
    let input = stage2_input(487, PlayerPosition::West);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 487");
    let premium_count = forced
        .cards
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    assert!(
        premium_count <= 1,
        "forced pass should not dump multiple premium hearts: {:?}",
        forced.cards
    );
    assert!(
        forced.cards.iter().any(|card| card.suit != Suit::Hearts),
        "forced pass should include an off-suit liability anchor: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_447_requires_anchor_support_for_queen() {
    let input = stage2_input(447, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 447");
    let support_count = forced
        .cards
        .iter()
        .filter(|card| is_high_support(card) || is_mid_support(card))
        .count();
    assert!(
        forced.cards.iter().any(|card| card.suit != Suit::Hearts),
        "forced pass should include an off-suit liability anchor: {:?}",
        forced.cards
    );
    assert!(
        support_count >= 1,
        "forced combo should retain at least one support heart: {:?}",
        forced.cards
    );
    assert!(
        forced
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten),
        "forced combo should still include a Ten+ heart for coverage: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_461_north_avoids_double_premium_dump() {
    let input = stage2_input(461, PlayerPosition::North);
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 461 north");
    let premium_count = forced
        .cards
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    assert!(
        premium_count <= 1,
        "forced pass should not ship multiple premium hearts: {:?}",
        forced.cards
    );
    assert!(
        forced.cards.iter().any(|card| card.suit != Suit::Hearts),
        "forced pass should include an off-suit anchor: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_767_forced_prefers_liability_mix() {
    let input = stage2_input(767, PlayerPosition::North);
    let enumerated = enumerate_pass_triples(&input);
    if enumerated.is_empty() {
        assert!(
            force_guarded_pass(&input).is_none(),
            "forced guard should remain unavailable when enumerator cannot build liability mix"
        );
    } else {
        for cand in enumerated {
            assert!(
                cand.cards.iter().any(|card| {
                    card.suit != Suit::Hearts && (card.is_queen_of_spades() || card.rank >= Rank::King)
                }),
                "enumerator output should include an off-suit liability: {:?}",
                cand.cards
            );
        }
        assert!(
            force_guarded_pass(&input).is_none(),
            "forced guard should stay empty once enumerator supplies liability mixes"
        );
    }
}

#[test]
fn stage2_hand_319_promotes_queen_liability_mix() {
    let input = stage2_input(319, PlayerPosition::North);
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.is_empty(),
        "expected enumerator to return candidates for hand 319"
    );
    let queen = Card::new(Rank::Queen, Suit::Hearts);
    let has_liability_combo = combos.iter().any(|candidate| {
        candidate.cards.contains(&queen)
            && candidate.cards.iter().any(|card| {
                card.suit != Suit::Hearts && (card.is_queen_of_spades() || card.rank >= Rank::King)
            })
    });
    assert!(
        has_liability_combo,
        "expected enumerator to surface Q♥ + liability mixes: {:?}",
        combos
            .iter()
            .take(5)
            .map(|candidate| candidate.cards)
            .collect::<Vec<[Card; 3]>>()
    );
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 319");
    assert!(
        forced.cards.contains(&queen),
        "forced fallback should ship Q♥: {:?}",
        forced.cards
    );
    assert!(
        forced.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.is_queen_of_spades() || card.rank >= Rank::King)
        }),
        "forced fallback should pair Q♥ with a high off-suit liability: {:?}",
        forced.cards
    );
}

#[test]
fn stage2_hand_761_prioritises_liability_anchor_for_queen() {
    let input = stage2_input(761, PlayerPosition::West);
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.is_empty(),
        "expected enumerator to return candidates for hand 761"
    );
    let top = &combos[0];
    assert!(
        top.cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen),
        "top candidate should include Q♥: {:?}",
        top.cards
    );
    assert!(
        top.cards.iter().any(|card| {
            card.suit != Suit::Hearts && (card.is_queen_of_spades() || card.rank >= Rank::King)
        }),
        "top candidate should anchor Q♥ with an off-suit liability: {:?}",
        top.cards
    );
    assert!(
        combos.iter().any(|candidate| {
            candidate
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen)
                && candidate.cards.iter().any(|card| {
                    card.suit != Suit::Hearts
                        && (card.is_queen_of_spades() || card.rank >= Rank::King)
                })
        }),
        "expected Q♥ candidates to include liability anchors: {:?}",
        combos
            .iter()
            .take(5)
            .map(|candidate| candidate.cards)
            .collect::<Vec<[Card; 3]>>()
    );
}

#[test]
fn stage2_hand_75_promotes_queen_with_single_support_anchor() {
    let input = stage2_input(75, PlayerPosition::West);
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.is_empty(),
        "expected enumerator to return candidates for hand 75"
    );
    let top = &combos[0];
    assert!(
        top.cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen),
        "top candidate should include Q♥: {:?}",
        top.cards
    );
    assert!(
        top.cards
            .iter()
            .any(|card| is_mid_support(card) || is_high_support(card)),
        "top candidate should include at least one support heart: {:?}",
        top.cards
    );
    assert!(
        !top.cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Ace),
        "top candidate should retain A♥: {:?}",
        top.cards
    );
    let forced = force_guarded_pass(&input).expect("expected forced candidate for hand 75");
    assert!(
        forced
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen),
        "forced fallback should also include Q♥: {:?}",
        forced.cards
    );
}

#[test]
fn fixture_hand_75_rejects_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    let invalid: Vec<[Card; 3]> = combos
        .iter()
        .filter(|candidate| {
            candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count()
                < 3
        })
        .map(|candidate| candidate.cards)
        .collect();
    assert!(
        invalid.is_empty(),
        "expected all combos to send three Ten+ hearts, found {invalid:?}"
    );
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Seven, Suit::Clubs),
                Card::new(Rank::Six, Suit::Clubs),
            ]
        )),
        "unexpected Q♠ dump candidate present"
    );
}

#[test]
fn fixture_hand_912_keeps_ace_with_insufficient_support() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.iter().all(|candidate| {
            !candidate
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Ace)
        }),
        "Ace should be retained when support is insufficient"
    );
}

#[test]
fn fixture_hand_498_requires_three_ten_plus_hearts() {
    let input = make_input(
        vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        PlayerPosition::East,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(combos.iter().all(|candidate| {
        !candidate
            .cards
            .iter()
            .any(|card| card.rank == Rank::King && card.suit == Suit::Hearts)
            || candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count()
                >= 3
    }));
}

#[test]
fn fixture_hand_511_rejects_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
        ],
        PlayerPosition::South,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Four, Suit::Clubs),
                Card::new(Rank::Five, Suit::Clubs),
            ]
        )),
        "unexpected Q♠ dump candidate present"
    );
}

#[test]
fn fixture_hand_767_promotes_heart_splits() {
    let input = make_input(
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Five, Suit::Clubs),
            ]
        )),
        "unexpected Q♠ dump candidate present"
    );

    for candidate in combos
        .iter()
        .filter(|candidate| candidate.cards.iter().any(|card| card.suit == Suit::Hearts))
    {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus > 0,
            "expected low-heart triple to include at least one Ten+ heart: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fallback_injects_best_available_heart_when_ten_plus_short() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Diamonds),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| *card == Card::new(Rank::Ace, Suit::Hearts))
            && candidate
                .cards
                .iter()
                .any(|card| *card == Card::new(Rank::Queen, Suit::Hearts))
    }) {
        assert_eq!(
            candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts)
                .count(),
            3,
            "expected fallback to promote an additional heart when Ten+ supply is short"
        );
        assert!(
            !candidate
                .cards
                .iter()
                .any(|card| card == &Card::new(Rank::King, Suit::Spades)),
            "fallback should replace K♠ with best available heart: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_32_requires_three_ten_plus_when_passing_qheart() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card == &Card::new(Rank::Queen, Suit::Hearts))
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "passing Q♥ must include three Ten+ hearts: {:?}",
            candidate.cards
        );
        assert!(
            !candidate.cards.contains(&Card::new(Rank::Ten, Suit::Clubs)),
            "expected Ten♣ to be replaced by a premium heart: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_567_requires_three_premium_hearts() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Clubs),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "passing Q♥/K♥/A♥ must include three Ten+ hearts: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_757_prevents_ace_club_anchor() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Ace, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Clubs),
            ]
        )),
        "expected guard to reject heart+club anchor pass"
    );
}

#[test]
fn fixture_hand_890_rejects_offsuit_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Jack, Suit::Spades),
                Card::new(Rank::Seven, Suit::Diamonds),
                Card::new(Rank::Six, Suit::Clubs),
            ]
        )),
        "expected off-suit dump to be rejected"
    );
}

#[test]
fn fixture_hand_153_requires_three_ten_plus_hearts() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ],
        PlayerPosition::South,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Two, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Hearts),
            ]
        )),
        "expected to avoid passing only two premium hearts with a low heart kicker"
    );
    for candidate in combos.iter().filter(|candidate| {
        candidate.cards.iter().any(|card| {
            card.suit == Suit::Hearts && (card.rank == Rank::Ace || card.rank == Rank::King)
        })
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "expected three Ten+ hearts when shipping A/K, got {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_242_blocks_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Two, Suit::Spades),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    let offenders: Vec<[Card; 3]> = combos
        .iter()
        .filter(|candidate| {
            candidate_contains(
                &candidate.cards,
                &[
                    Card::new(Rank::Queen, Suit::Spades),
                    Card::new(Rank::Seven, Suit::Clubs),
                    Card::new(Rank::Eight, Suit::Clubs),
                ],
            )
        })
        .map(|candidate| candidate.cards)
        .collect();
    assert!(
        offenders.is_empty(),
        "unexpected Q♠ + clubs dump persisted: {offenders:?}"
    );
}

#[test]
fn fixture_hand_432_requires_ten_plus_substitution() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
        ],
        PlayerPosition::East,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Four, Suit::Hearts),
                Card::new(Rank::Jack, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
            ]
        )),
        "expected low-heart kicker to be replaced by Ten+ support"
    );
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::King)
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "expected replacement Ten+ heart present, got {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_461_rejects_qspade_ace_combo() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Ace, Suit::Hearts),
            ]
        )),
        "expected Ace guard to reject Q♠ + club pass"
    );
}

#[test]
fn fixture_hand_681_rejects_qspade_ace_combo() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Five, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Ace, Suit::Hearts),
            ]
        )),
        "expected Ace guard to reject Q♠ + club pass"
    );
}

#[test]
fn fixture_hand_757_rejects_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Seven, Suit::Clubs),
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Spades),
            ]
        )),
        "unexpected Q♠ + clubs dump persisted"
    );
}

#[test]
#[ignore]
fn debug_stage2_hand_610_north() {
    let input = stage2_input(610, PlayerPosition::North);
    println!("hand 610 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
        let support_high = cand
            .cards
            .iter()
            .filter(|card| is_high_support(card))
            .count();
        let support_mid = cand
            .cards
            .iter()
            .filter(|card| is_mid_support(card))
            .count();
        let passes_j = cand
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Jack);
        if passes_j {
            println!(
                "    support_high={}, support_mid={}, support_total={}",
                support_high,
                support_mid,
                support_high + support_mid
            );
        }
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
    let hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    let queen = hearts.iter().find(|card| card.rank == Rank::Queen).copied();
    let jack = hearts.iter().find(|card| card.rank == Rank::Jack).copied();
    if let (Some(q), Some(j)) = (queen, jack) {
        let support_remaining = hearts
            .iter()
            .filter(|card| {
                card.rank >= Rank::Ten && card.rank < Rank::Queen && **card != q && **card != j
            })
            .count()
            + hearts
                .iter()
                .filter(|card| {
                    card.suit == Suit::Hearts
                        && card.rank >= Rank::Eight
                        && card.rank < Rank::Ten
                        && **card != q
                        && **card != j
                })
                .count();
        println!("support_remaining={}", support_remaining);
        if support_remaining == 0 {
            let mut off_cards: Vec<Card> = input
                .hand
                .iter()
                .copied()
                .filter(|card| card.suit != Suit::Hearts)
                .collect();
            off_cards.sort_by(|a, b| a.rank.value().cmp(&b.rank.value()));
            if let Some(off) = off_cards.first() {
                println!("manual combo: {:?} {:?} {:?}", q, j, off);
            }
        }
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_44_south() {
    let input = stage2_input(44, PlayerPosition::South);
    println!("hand 44 south:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_380_south() {
    let input = stage2_input(380, PlayerPosition::South);
    println!("hand 380 south:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => {
            println!("forced: {:?}", candidate.cards);
            for card in candidate.cards.iter() {
                println!("  passed {:?}", card);
            }
            let support_available = input
                .hand
                .iter()
                .filter(|card| {
                    card.suit == Suit::Hearts
                        && (is_high_support(card) || is_mid_support(card))
                        && !candidate.cards.contains(card)
                })
                .count();
            let remaining: Vec<_> = input
                .hand
                .iter()
                .filter(|card| !candidate.cards.contains(card))
                .collect();
            println!("support_available={}", support_available);
            println!("remaining: {:?}", remaining);
        }
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_767_north() {
    let input = stage2_input(767, PlayerPosition::North);
    println!("hand 767 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_757_north() {
    let input = stage2_input(757, PlayerPosition::North);
    println!("hand 757 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_119_north() {
    let input = stage2_input(119, PlayerPosition::North);
    println!("hand 119 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_310_north() {
    let input = stage2_input(310, PlayerPosition::North);
    println!("hand 310 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_problem_hands() {
    let hands = [119, 310, 367, 462, 464, 481, 597, 607, 852, 865, 887];
    for &hand in &hands {
        let input = stage2_input(hand, PlayerPosition::North);
        println!("=== hand {} north ===", hand);
        for card in input.hand.iter() {
            println!("  {:?}", card);
        }
        let combos = enumerate_pass_triples(&input);
        println!("enumerated {} combos:", combos.len());
        for cand in combos.iter() {
            println!("  {:?} score={:.2}", cand.cards, cand.score);
        }
        match force_guarded_pass(&input) {
            Some(candidate) => println!("forced: {:?}\n", candidate.cards),
            None => println!("forced: None\n"),
        }
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_941_east() {
    let input = stage2_input(941, PlayerPosition::East);
    println!("hand 941 east:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_594_north() {
    let input = stage2_input(594, PlayerPosition::North);
    println!("hand 594 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_447_north() {
    let input = stage2_input(447, PlayerPosition::North);
    println!("hand 447 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_699_north() {
    let input = stage2_input(699, PlayerPosition::North);
    println!("hand 699 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    let mut strong_spades: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.is_queen_of_spades() || (card.suit == Suit::Spades && card.rank >= Rank::King)
        })
        .collect();
    strong_spades.sort_by(|a, b| b.rank.cmp(&a.rank));
    strong_spades.dedup();
    println!("strong spades: {:?}", strong_spades);
    let mut hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    hearts.sort_by(|a, b| a.rank.cmp(&b.rank));
    println!("sorted hearts: {:?}", hearts);
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_526_north() {
    let input = stage2_input(526, PlayerPosition::North);
    println!("hand 526 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_904_west() {
    let input = stage2_input(904, PlayerPosition::West);
    println!("hand 904 west:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_912_north() {
    let input = stage2_input(912, PlayerPosition::North);
    println!("hand 912 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_461_all() {
    for seat in [
        PlayerPosition::North,
        PlayerPosition::East,
        PlayerPosition::South,
        PlayerPosition::West,
    ] {
        let input = stage2_input(461, seat);
        println!("hand 461 {:?}:", seat);
        for card in input.hand.iter() {
            println!("  {:?}", card);
        }
        let combos = enumerate_pass_triples(&input);
        println!("enumerated {} combos:", combos.len());
        for cand in combos.iter() {
            println!("  {:?} score={:.2}", cand.cards, cand.score);
        }
        match force_guarded_pass(&input) {
            Some(candidate) => println!("forced: {:?}", candidate.cards),
            None => println!("forced: None"),
        }
        println!();
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_319_north() {
    let input = stage2_input(319, PlayerPosition::North);
    println!("hand 319 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_487_west() {
    let input = stage2_input(487, PlayerPosition::West);
    println!("hand 487 west:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_320_east() {
    let input = stage2_input(320, PlayerPosition::East);
    println!("hand 320 east:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_64_west() {
    let input = stage2_input(64, PlayerPosition::West);
    println!("hand 64 west:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_75_west() {
    let input = stage2_input(75, PlayerPosition::West);
    println!("hand 75 west:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_26_north() {
    let input = stage2_input(26, PlayerPosition::North);
    println!("hand 26 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_96_north() {
    let input = stage2_input(96, PlayerPosition::North);
    println!("hand 96 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_212_north() {
    let input = stage2_input(212, PlayerPosition::North);
    println!("hand 212 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_420_north() {
    let input = stage2_input(420, PlayerPosition::North);
    println!("hand 420 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_599_north() {
    let input = stage2_input(599, PlayerPosition::North);
    println!("hand 599 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_928_north() {
    let input = stage2_input(928, PlayerPosition::North);
    println!("hand 928 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}

#[test]
#[ignore]
fn debug_stage2_hand_941_north() {
    let input = stage2_input(941, PlayerPosition::North);
    println!("hand 941 north:");
    for card in input.hand.iter() {
        println!("  {:?}", card);
    }
    let combos = enumerate_pass_triples(&input);
    println!("enumerated {} combos:", combos.len());
    for cand in combos.iter() {
        println!("  {:?} score={:.2}", cand.cards, cand.score);
    }
    match force_guarded_pass(&input) {
        Some(candidate) => println!("forced: {:?}", candidate.cards),
        None => println!("forced: None"),
    }
}
