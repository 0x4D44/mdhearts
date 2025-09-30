use hearts_core::game::match_state::MatchState;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayError, PlayOutcome, RoundPhase};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::core::PCWSTR;

pub struct GameController {
    match_state: MatchState,
    last_trick: Option<TrickSummary>,
}

impl GameController {
    fn dbg(msg: &str) {
        let mut wide: Vec<u16> = msg.encode_utf16().collect();
        wide.push(0);
        unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())); }
    }
    pub fn new_with_seed(seed: Option<u64>, starting: PlayerPosition) -> Self {
        let match_state = if let Some(s) = seed {
            MatchState::with_seed(starting, s)
        } else {
            MatchState::new(starting)
        };
        Self { match_state, last_trick: None }
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
        }?;

        if let PlayOutcome::TrickCompleted { winner, .. } = out {
            let mut plays = pre_plays;
            plays.push((seat, card));
            self.last_trick = Some(TrickSummary { winner, plays });
        }

        // Defer end-of-round auto-advance to the UI so we can finish
        // trick collect animations before dealing the next round.

        Ok(out)
    }

    pub fn in_passing_phase(&self) -> bool {
        matches!(self.match_state.round().phase(), RoundPhase::Passing(_))
    }

    pub fn submit_pass(
        &mut self,
        seat: PlayerPosition,
        cards: [Card; 3],
    ) -> Result<(), hearts_core::model::passing::PassingError> {
        self.match_state.round_mut().submit_pass(seat, cards)
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
        if seat == stop_seat { return None; }
        let legal = self.legal_moves(seat);
        if let Some(card) = legal.first().copied() {
            Self::dbg(&format!("mdhearts: AI {:?} plays {}", seat, card));
            let _ = self.play(seat, card);
            Some((seat, card))
        } else {
            None
        }
    }

    pub fn hand(&self, seat: PlayerPosition) -> Vec<Card> {
        self.match_state.round().hand(seat).iter().copied().collect()
    }

    pub fn legal_moves_set(&self, seat: PlayerPosition) -> std::collections::HashSet<Card> {
        use std::collections::HashSet;
        self.legal_moves(seat).into_iter().collect::<HashSet<_>>()
    }

    pub fn trick_leader(&self) -> PlayerPosition {
        self.match_state.round().current_trick().leader()
    }

    pub fn trick_plays(&self) -> Vec<(PlayerPosition, Card)> {
        self
            .match_state
            .round()
            .current_trick()
            .plays()
            .iter()
            .map(|p| (p.position, p.card))
            .collect()
    }

    pub fn simple_pass_for(&self, seat: PlayerPosition) -> Option<[Card; 3]> {
        let hand = self.match_state.round().hand(seat);
        if hand.len() < 3 {
            return None;
        }
        let picks = [hand.cards()[0], hand.cards()[1], hand.cards()[2]];
        Some(picks)
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
                self.match_state.round_mut().submit_pass(seat, cards)?;
            }
        }
        Ok(())
    }

    pub fn restart_round(&mut self) {
        let seed = self.match_state.seed();
        let round_num = self.match_state.round_number();
        let passing = self.match_state.passing_direction();
        let starting = self.match_state.round().starting_player();
        self.match_state = MatchState::with_seed_round_direction(
            seed,
            round_num,
            passing,
            starting,
        );
    }

    pub fn finish_round_if_ready(&mut self) {
        if self.match_state.is_round_ready_for_scoring() {
            self.match_state.finish_round_and_start_next();
        }
    }
}

