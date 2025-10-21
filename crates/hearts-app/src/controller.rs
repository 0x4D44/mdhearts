use crate::bot::MoonState;
use crate::bot::{BotContext, BotDifficulty, PassPlanner, PlayPlanner, UnseenTracker};
use hearts_core::game::match_state::MatchState;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{PlayError, PlayOutcome, RoundPhase};
use hearts_core::model::suit::Suit;
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
#[cfg(windows)]
use windows::core::PCWSTR;

pub struct GameController {
    match_state: MatchState,
    last_trick: Option<TrickSummary>,
    bot_difficulty: BotDifficulty,
    unseen_tracker: UnseenTracker,
}

impl GameController {
    fn dbg(msg: &str) {
        fn debug_enabled() -> bool {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *ON.get_or_init(|| {
                std::env::var("MDH_DEBUG_LOGS")
                    .map(|v| {
                        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
                    })
                    .unwrap_or(false)
            })
        }
        if !debug_enabled() {
            return;
        }
        #[cfg(windows)]
        {
            let mut wide: Vec<u16> = msg.encode_utf16().collect();
            wide.push(0);
            unsafe {
                OutputDebugStringW(PCWSTR(wide.as_ptr()));
            }
        }
        #[cfg(not(windows))]
        {
            eprintln!("{msg}");
        }
    }
    pub fn new_with_seed(seed: Option<u64>, starting: PlayerPosition) -> Self {
        let match_state = if let Some(s) = seed {
            MatchState::with_seed(starting, s)
        } else {
            MatchState::new(starting)
        };
        let mut unseen_tracker = UnseenTracker::new();
        unseen_tracker.reset_for_round(match_state.round());
        let this = Self {
            match_state,
            last_trick: None,
            bot_difficulty: BotDifficulty::from_env(),
            unseen_tracker,
        };
        Self::dbg(&format!(
            "mdhearts: AI weights {}",
            crate::bot::debug_weights_string()
        ));
        this
    }

    pub fn new_from_match_state(match_state: hearts_core::game::match_state::MatchState) -> Self {
        let mut unseen_tracker = UnseenTracker::new();
        unseen_tracker.reset_for_round(match_state.round());
        let this = Self {
            match_state,
            last_trick: None,
            bot_difficulty: BotDifficulty::from_env(),
            unseen_tracker,
        };
        Self::dbg(&format!(
            "mdhearts: AI weights {}",
            crate::bot::debug_weights_string()
        ));
        this
    }

    fn bot_context(&self, seat: PlayerPosition) -> BotContext<'_> {
        BotContext::new(
            seat,
            self.match_state.round(),
            self.match_state.scores(),
            self.match_state.passing_direction(),
            &self.unseen_tracker,
            self.bot_difficulty,
        )
    }

    pub fn set_bot_difficulty(&mut self, difficulty: BotDifficulty) {
        self.bot_difficulty = difficulty;
    }

    pub fn bot_difficulty(&self) -> BotDifficulty {
        self.bot_difficulty
    }

    #[cfg(test)]
    fn configure_for_test(&mut self) {
        self.bot_difficulty = BotDifficulty::NormalHeuristic;
        self.unseen_tracker
            .reset_for_round(self.match_state.round());
    }
    pub fn status_text(&self) -> String {
        let round = self.match_state.round();
        let passing = self.match_state.passing_direction().as_str();
        let leader = round.current_trick().leader();
        format!(
            "Round {} • Passing: {} • Leader: {}",
            self.match_state.round_number(),
            passing,
            leader
        )
    }

    pub fn legal_moves(&self, seat: PlayerPosition) -> Vec<Card> {
        let round = self.match_state.round();
        let hand = round.hand(seat);
        hand.iter()
            .copied()
            .filter(|&card| {
                let mut probe = round.clone();
                probe.play_card(seat, card).is_ok()
            })
            .collect()
    }

    pub fn play(&mut self, seat: PlayerPosition, card: Card) -> Result<PlayOutcome, PlayError> {
        // Snapshot trick before applying the play so we can reconstruct on completion
        let pre_plays: Vec<(PlayerPosition, Card)> = {
            let round = self.match_state.round();
            round
                .current_trick()
                .plays()
                .iter()
                .map(|p| (p.position, p.card))
                .collect()
        };
        let out = {
            let round = self.match_state.round_mut();
            round.play_card(seat, card)
        };

        let out = match out {
            Ok(value) => {
                // Track card reveal
                self.unseen_tracker.note_card_played(seat, card);
                // If this was a follow where suit was not followed, we can deduce a void.
                if let Some(lead_suit) = pre_plays.first().map(|p| p.1.suit)
                    && card.suit != lead_suit
                {
                    self.unseen_tracker.note_void(seat, lead_suit);
                }
                value
            }
            Err(err) => return Err(err),
        };

        if let PlayOutcome::TrickCompleted { winner, penalties } = out {
            let mut plays = pre_plays;
            plays.push((seat, card));
            self.last_trick = Some(TrickSummary { winner, plays });
            // Update moon state heuristics for the winner and others.
            self.update_moon_states_after_trick(winner, penalties);
        }

        // Defer end-of-round auto-advance to the UI so we can finish
        // trick collect animations before dealing the next round.

        Ok(out)
    }

    fn update_moon_states_after_trick(&mut self, winner: PlayerPosition, penalties: u8) {
        // Stage 2 heuristic: commit if a seat shows sustained control early with low penalties; abort on loss of control or fragmented points.
        let round = self.match_state.round();
        let scores = self.match_state.scores();
        let cards_played = 52usize.saturating_sub(self.unseen_tracker.unseen_count());
        let totals = round.penalty_totals();
        use hearts_core::model::rank::Rank;
        use hearts_core::model::suit::Suit;

        for seat in PlayerPosition::LOOP.iter().copied() {
            let state = self.unseen_tracker.moon_state(seat);
            // Commit gate: winner of a clean, early trick, with good hearts capacity and some control history.
            if (state == MoonState::Inactive || state == MoonState::Considering)
                && seat == winner
                && penalties == 0
                && cards_played <= 20
                && scores.score(seat) < 70
            {
                let tricks_won = {
                    let mut counts = [0u8; 4];
                    for trick in round.trick_history() {
                        if let Some(w) = trick.winner() {
                            counts[w.index()] = counts[w.index()].saturating_add(1);
                        }
                    }
                    counts[seat.index()]
                };
                let hand = round.hand(seat);
                let hearts_in_hand = hand.iter().filter(|c| c.suit == Suit::Hearts).count();
                let control_hearts = hand
                    .iter()
                    .filter(|c| c.suit == Suit::Hearts && c.rank >= Rank::Ten)
                    .count();
                if tricks_won >= 1 && hearts_in_hand >= 5 && control_hearts >= 3 {
                    let next = if state == MoonState::Considering {
                        MoonState::Committed
                    } else {
                        MoonState::Considering
                    };
                    Self::dbg(&format!(
                        "mdhearts: moon {:?} -> {:?} (seat={:?}, tricks_won={}, hearts={}, control_hearts={})",
                        state, next, seat, tricks_won, hearts_in_hand, control_hearts
                    ));
                    self.unseen_tracker.set_moon_state(seat, next);
                }
            }

            // Abort conditions for committed moon: lost control, opponents collected hearts, near-end, or too few hearts left
            if state == MoonState::Committed {
                let my_total = totals[seat.index()];
                let others_hearts: u32 = PlayerPosition::LOOP
                    .iter()
                    .copied()
                    .filter(|&p| p != seat)
                    .map(|p| totals[p.index()] as u32)
                    .sum();
                let near_end = cards_played >= 36;
                let lost_control_recent = winner != seat && penalties == 0; // failed to capture a clean trick
                let hearts_left = round
                    .hand(seat)
                    .iter()
                    .filter(|c| c.suit == Suit::Hearts)
                    .count();
                if others_hearts >= 3 || near_end || lost_control_recent || hearts_left < 3 {
                    Self::dbg(&format!(
                        "mdhearts: moon ABORT for {:?} (others_hearts={}, near_end={}, lost_control={}, hearts_left={}, my_total={})",
                        seat, others_hearts, near_end, lost_control_recent, hearts_left, my_total
                    ));
                    self.unseen_tracker
                        .set_moon_state(seat, MoonState::Inactive);
                }
            }
        }
    }

    pub fn in_passing_phase(&self) -> bool {
        matches!(self.match_state.round().phase(), RoundPhase::Passing(_))
    }

    pub fn submit_pass(
        &mut self,
        seat: PlayerPosition,
        cards: [Card; 3],
    ) -> Result<(), hearts_core::model::passing::PassingError> {
        let result = self.match_state.round_mut().submit_pass(seat, cards);
        if result.is_ok() {
            self.unseen_tracker.note_pass_selection(seat, &cards);
        }
        result
    }

    pub fn resolve_passes(&mut self) -> Result<(), hearts_core::model::passing::PassingError> {
        self.match_state.round_mut().resolve_passes()
    }

    pub fn standings(&self) -> [u32; 4] {
        *self.match_state.scores().standings()
    }

    pub fn penalties_this_round(&self) -> [u8; 4] {
        self.match_state.round_penalties()
    }

    pub fn tricks_won_this_round(&self) -> [u8; 4] {
        let mut counts = [0u8; 4];
        let round = self.match_state.round();
        for trick in round.trick_history() {
            if let Some(w) = trick.winner() {
                let idx = w.index();
                counts[idx] = counts[idx].saturating_add(1);
            }
        }
        counts
    }

    pub fn passing_direction(&self) -> hearts_core::model::passing::PassingDirection {
        self.match_state.passing_direction()
    }

    pub fn explain_candidates_for(&self, seat: PlayerPosition) -> Vec<(Card, i32)> {
        let legal = self.legal_moves(seat);
        let ctx = self.bot_context(seat);
        crate::bot::PlayPlanner::explain_candidates(&legal, &ctx)
    }

    // Test-only helpers
    #[cfg(test)]
    pub fn set_round_and_scores_for_test(
        &mut self,
        round: hearts_core::model::round::RoundState,
        scores: [u32; 4],
    ) {
        *self.match_state.round_mut() = round;
        self.match_state.scores_mut().set_totals(scores);
        self.unseen_tracker.reset_for_round(self.match_state.round());
    }

    #[cfg(test)]
    pub fn moon_state_for_test(&self, seat: PlayerPosition) -> crate::bot::MoonState {
        self.unseen_tracker.moon_state(seat)
    }
}

#[derive(Debug, Clone)]
pub struct TrickSummary {
    pub winner: PlayerPosition,
    pub plays: Vec<(PlayerPosition, Card)>,
}

impl GameController {
    pub fn expected_to_play(&self) -> PlayerPosition {
        let trick = self.match_state.round().current_trick();
        trick
            .plays()
            .last()
            .map(|p| p.position.next())
            .unwrap_or(trick.leader())
    }

    pub fn take_last_trick_summary(&mut self) -> Option<TrickSummary> {
        self.last_trick.take()
    }

    pub fn last_trick(&self) -> Option<&TrickSummary> {
        self.last_trick.as_ref()
    }

    // Play a single AI move (if it's not stop_seat's turn). Returns the (seat, card) played.
    pub fn autoplay_one(&mut self, stop_seat: PlayerPosition) -> Option<(PlayerPosition, Card)> {
        if self.in_passing_phase() {
            return None;
        }
        let seat = self.expected_to_play();
        if seat == stop_seat {
            return None;
        }
        let enforce_two = {
            let round = self.match_state.round();
            round.is_first_trick() && round.current_trick().leader() == seat
        };
        let legal = self.legal_moves(seat);
        let card_to_play = if enforce_two {
            let two = Card::new(Rank::Two, Suit::Clubs);
            if legal.contains(&two) {
                Some(two)
            } else {
                legal.first().copied()
            }
        } else {
            match self.bot_difficulty {
                BotDifficulty::EasyLegacy => legal.first().copied(),
                _ => {
                    // Compute style without holding an immutable borrow across mutation
                    let commit = {
                        let ctx_probe = self.bot_context(seat);
                        crate::bot::determine_style(&ctx_probe)
                            == crate::bot::BotStyle::AggressiveMoon
                    };
                    if commit {
                        self.unseen_tracker
                            .set_moon_state(seat, MoonState::Committed);
                    }
                    let ctx = self.bot_context(seat);
                    PlayPlanner::choose(&legal, &ctx).or_else(|| legal.first().copied())
                }
            }
        };
        if let Some(card) = card_to_play {
            Self::dbg(&format!("mdhearts: AI {:?} plays {}", seat, card));
            let _ = self.play(seat, card);
            Some((seat, card))
        } else {
            None
        }
    }
    pub fn hand(&self, seat: PlayerPosition) -> Vec<Card> {
        self.match_state
            .round()
            .hand(seat)
            .iter()
            .copied()
            .collect()
    }

    pub fn legal_moves_set(&self, seat: PlayerPosition) -> std::collections::HashSet<Card> {
        use std::collections::HashSet;
        self.legal_moves(seat).into_iter().collect::<HashSet<_>>()
    }

    pub fn trick_leader(&self) -> PlayerPosition {
        self.match_state.round().current_trick().leader()
    }

    pub fn trick_plays(&self) -> Vec<(PlayerPosition, Card)> {
        self.match_state
            .round()
            .current_trick()
            .plays()
            .iter()
            .map(|p| (p.position, p.card))
            .collect()
    }

    pub fn simple_pass_for(&self, seat: PlayerPosition) -> Option<[Card; 3]> {
        let hand = self.match_state.round().hand(seat);
        match self.bot_difficulty {
            BotDifficulty::EasyLegacy => {
                if hand.len() < 3 {
                    return None;
                }
                Some([hand.cards()[0], hand.cards()[1], hand.cards()[2]])
            }
            _ => {
                let ctx = self.bot_context(seat);
                PassPlanner::choose(hand, &ctx)
            }
        }
    }

    pub fn submit_auto_passes_for_others(
        &mut self,
        except: PlayerPosition,
    ) -> Result<(), hearts_core::model::passing::PassingError> {
        for seat in PlayerPosition::LOOP.iter().copied() {
            if seat == except {
                continue;
            }
            if let Some(cards) = self.simple_pass_for(seat) {
                self.submit_pass(seat, cards)?;
            }
        }
        Ok(())
    }

    pub fn restart_round(&mut self) {
        let seed = self.match_state.seed();
        let round_num = self.match_state.round_number();
        let passing = self.match_state.passing_direction();
        let starting = self.match_state.round().starting_player();
        self.match_state =
            MatchState::with_seed_round_direction(seed, round_num, passing, starting);
        self.unseen_tracker
            .reset_for_round(self.match_state.round());
    }

    pub fn finish_round_if_ready(&mut self) {
        if self.match_state.is_round_ready_for_scoring() {
            self.match_state.finish_round_and_start_next();
            self.unseen_tracker
                .reset_for_round(self.match_state.round());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::GameController;
    use crate::bot::BotDifficulty;
    use hearts_core::model::card::Card;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::suit::Suit;

    #[test]
    fn easy_legacy_pass_returns_first_three() {
        let mut controller = GameController::new_with_seed(Some(42), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::EasyLegacy);

        let seat = PlayerPosition::East;
        let hand = controller.hand(seat);
        assert!(hand.len() >= 3);
        let expected = [hand[0], hand[1], hand[2]];
        let actual = controller.simple_pass_for(seat).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn easy_legacy_autoplay_uses_first_card() {
        let mut controller = GameController::new_with_seed(Some(123), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::EasyLegacy);

        if controller.in_passing_phase() {
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();
        }

        let seat = controller.expected_to_play();
        let legal = controller.legal_moves(seat);
        assert!(!legal.is_empty());

        let result = controller.autoplay_one(PlayerPosition::South).unwrap();
        assert_eq!(result.0, seat);
        assert_eq!(result.1, legal[0]);
    }

    #[test]
    fn scripted_round_cautious_lead_after_passes() {
        let mut controller = GameController::new_with_seed(Some(31415), PlayerPosition::North);
        controller.configure_for_test();

        if controller.in_passing_phase() {
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();
        }

        while !controller.in_passing_phase()
            && controller.expected_to_play() != PlayerPosition::South
        {
            if controller.autoplay_one(PlayerPosition::South).is_none() {
                break;
            }
        }

        assert_eq!(controller.expected_to_play(), PlayerPosition::South);
        let legal = controller.legal_moves(PlayerPosition::South);
        assert!(!legal.is_empty());
        let has_high_heart = legal
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen);

        if has_high_heart {
            let (_, played) = controller.autoplay_one(PlayerPosition::North).unwrap();
            assert_ne!(played.suit, Suit::Hearts);
        }

        while controller.autoplay_one(PlayerPosition::North).is_some() {
            if controller.in_passing_phase() {
                break;
            }
        }
    }
    #[test]
    fn first_ai_play_after_passes_is_two_of_clubs() {
        for seed in 0u64..1024 {
            let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
            if controller.passing_direction() == PassingDirection::Hold {
                continue;
            }
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();

            let two = Card::new(Rank::Two, Suit::Clubs);
            let holder = PlayerPosition::LOOP
                .iter()
                .copied()
                .find(|seat| controller.hand(*seat).contains(&two))
                .expect("two of clubs dealt");
            assert_eq!(
                controller.trick_leader(),
                holder,
                "seed {} leader should hold 2C",
                seed
            );

            let mut first = None;
            loop {
                if controller.in_passing_phase() {
                    break;
                }
                let seat = controller.expected_to_play();
                if seat == PlayerPosition::South {
                    break;
                }
                match controller.autoplay_one(PlayerPosition::South) {
                    Some(play) => {
                        first.get_or_insert(play);
                    }
                    None => break,
                }
            }

            if holder == PlayerPosition::South {
                let legal = controller.legal_moves(PlayerPosition::South);
                assert_eq!(legal.len(), 1, "seed {} south legal count", seed);
                assert_eq!(
                    legal[0],
                    Card::new(Rank::Two, Suit::Clubs),
                    "seed {} south must hold 2C",
                    seed
                );
            } else if let Some((_, card)) = first {
                assert_eq!(
                    card,
                    Card::new(Rank::Two, Suit::Clubs),
                    "seed {} should lead with 2C",
                    seed
                );
            }
        }
    }

    #[test]
    fn moon_commit_then_abort_flow() {
        let mut controller = GameController::new_with_seed(Some(12345), PlayerPosition::South);
        controller.configure_for_test();

        // Hands setup
        let south = vec![
            // Hearts control
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            // Clubs for clean captures
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Clubs),
            // A low diamond to slough
            Card::new(Rank::Two, Suit::Diamonds),
        ];
        let east = vec![
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Diamonds), // later wins clean diamond trick
        ];
        let north = vec![
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
        ];
        let west = vec![
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
        ];

        let round = {
            use hearts_core::model::hand::Hand;
            use hearts_core::model::round::{RoundPhase, RoundState};
            use hearts_core::model::trick::Trick;
            let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
            hands[PlayerPosition::South.index()] = Hand::with_cards(south);
            hands[PlayerPosition::East.index()] = Hand::with_cards(east);
            hands[PlayerPosition::North.index()] = Hand::with_cards(north);
            hands[PlayerPosition::West.index()] = Hand::with_cards(west);
            // Seed a completed trick to avoid first-trick (2C) restrictions
            let mut seed = Trick::new(PlayerPosition::South);
            seed.play(PlayerPosition::South, Card::new(Rank::Two, Suit::Clubs)).unwrap();
            seed.play(PlayerPosition::West, Card::new(Rank::Three, Suit::Clubs)).unwrap();
            seed.play(PlayerPosition::North, Card::new(Rank::Four, Suit::Clubs)).unwrap();
            seed.play(PlayerPosition::East, Card::new(Rank::Five, Suit::Clubs)).unwrap();
            RoundState::from_hands_with_state(
                hands,
                PlayerPosition::South,
                PassingDirection::Hold,
                RoundPhase::Playing,
                Trick::new(PlayerPosition::South),
                vec![seed],
                false,
            )
        };
        controller.set_round_and_scores_for_test(round, [10, 20, 10, 15]);

        // Trick 1 (clubs): South AC, then West 5C, North 7C, East 6C -> South wins clean
        let mut t1 = vec![
            (PlayerPosition::South, Card::new(Rank::Ace, Suit::Clubs)),
            (PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs)),
            (PlayerPosition::North, Card::new(Rank::Seven, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Six, Suit::Clubs)),
        ];
        for _ in 0..4 {
            let seat = controller.expected_to_play();
            let idx = t1
                .iter()
                .position(|(s, _)| *s == seat)
                .expect("seat to play present");
            let (_, card) = t1.remove(idx);
            let legal = controller.legal_moves(seat);
            assert!(legal.contains(&card), "illegal {:?} for {:?}", card, seat);
            controller.play(seat, card).expect("play ok");
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Considering as u8
        );

        // Trick 2: South KC, West 6C, North 3D (void), East 8C -> South still wins clean
        let mut t2 = vec![
            (PlayerPosition::South, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::West, Card::new(Rank::Six, Suit::Clubs)),
            (PlayerPosition::North, Card::new(Rank::Three, Suit::Diamonds)),
            (PlayerPosition::East, Card::new(Rank::Eight, Suit::Clubs)),
        ];
        for _ in 0..4 {
            let seat = controller.expected_to_play();
            let idx = t2
                .iter()
                .position(|(s, _)| *s == seat)
                .expect("seat to play present");
            let (_, card) = t2.remove(idx);
            let legal = controller.legal_moves(seat);
            assert!(legal.contains(&card), "illegal {:?} for {:?}", card, seat);
            controller.play(seat, card).expect("play ok");
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Committed as u8
        );

        // Trick 3 (diamonds): South 2D, West 4D, North 3D, East AD -> East wins clean => abort
        let mut t3 = vec![
            (PlayerPosition::South, Card::new(Rank::Two, Suit::Diamonds)),
            (PlayerPosition::West, Card::new(Rank::Four, Suit::Diamonds)),
            (PlayerPosition::North, Card::new(Rank::Six, Suit::Diamonds)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Diamonds)),
        ];
        for _ in 0..4 {
            let seat = controller.expected_to_play();
            let idx = t3
                .iter()
                .position(|(s, _)| *s == seat)
                .expect("seat to play present");
            let (_, card) = t3.remove(idx);
            let legal = controller.legal_moves(seat);
            assert!(legal.contains(&card), "illegal {:?} for {:?}", card, seat);
            controller.play(seat, card).expect("play ok");
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Inactive as u8
        );
    }
}
