use crate::bot::UnseenTracker;
use crate::policy::PolicyContext;
use hearts_core::model::card::Card;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

/// Observation schema version
#[allow(dead_code)]
pub const SCHEMA_VERSION: &str = env!("SCHEMA_VERSION");

/// Schema hash computed at build time
#[allow(dead_code)]
pub const SCHEMA_HASH: &str = env!("SCHEMA_HASH");

/// Total observation feature dimension
#[allow(dead_code)]
pub const FEATURE_DIM: usize = 270;

/// Fixed-size observation vector for neural network input
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Observation {
    // === Hand features (52) ===
    pub hand_onehot: [f32; 52],

    // === Seen cards (52) ===
    pub seen_onehot: [f32; 52],

    // === Current trick (75) ===
    pub trick_led_suit: [f32; 4],
    pub trick_cards: [[f32; 17]; 4], // 68 features
    pub trick_count: f32,
    pub my_trick_position: f32,
    pub trick_pad: f32, // Pad to 75 (4+68+1+1+1=75)

    // === Scores (4) ===
    pub scores_relative: [f32; 4],

    // === Game state (7) ===
    pub hearts_broken: f32,
    pub tricks_completed: f32,
    pub passing_phase: f32,
    pub passing_direction: [f32; 4],

    // === Opponent void inference (12) ===
    pub opp_voids: [f32; 12],

    // === History (68) ===
    pub last_4_cards: [f32; 68],
}

impl Observation {
    /// Flatten observation into a single array for neural network input
    #[allow(dead_code)]
    pub fn as_array(&self) -> [f32; FEATURE_DIM] {
        let mut arr = [0.0f32; FEATURE_DIM];
        let mut offset = 0;

        // Hand one-hot (52)
        arr[offset..offset + 52].copy_from_slice(&self.hand_onehot);
        offset += 52;

        // Seen one-hot (52)
        arr[offset..offset + 52].copy_from_slice(&self.seen_onehot);
        offset += 52;

        // Trick features (75)
        arr[offset..offset + 4].copy_from_slice(&self.trick_led_suit);
        offset += 4;
        for card_features in &self.trick_cards {
            arr[offset..offset + 17].copy_from_slice(card_features);
            offset += 17;
        }
        arr[offset] = self.trick_count;
        offset += 1;
        arr[offset] = self.my_trick_position;
        offset += 1;
        arr[offset] = self.trick_pad;
        offset += 1;

        // Scores (4)
        arr[offset..offset + 4].copy_from_slice(&self.scores_relative);
        offset += 4;

        // Game state (7)
        arr[offset] = self.hearts_broken;
        offset += 1;
        arr[offset] = self.tricks_completed;
        offset += 1;
        arr[offset] = self.passing_phase;
        offset += 1;
        arr[offset..offset + 4].copy_from_slice(&self.passing_direction);
        offset += 4;

        // Opponent voids (12)
        arr[offset..offset + 12].copy_from_slice(&self.opp_voids);
        offset += 12;

        // History (68)
        arr[offset..offset + 68].copy_from_slice(&self.last_4_cards);
        offset += 68;

        debug_assert_eq!(offset, FEATURE_DIM);
        arr
    }
}

/// Builder for creating observations from game state
pub struct ObservationBuilder;

impl ObservationBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build observation from policy context
    #[allow(dead_code)]
    pub fn build(&self, ctx: &PolicyContext) -> Observation {
        let (trick_led_suit, trick_cards, trick_count, my_trick_position) =
            self.encode_trick(ctx.round, ctx.seat);

        Observation {
            hand_onehot: self.encode_hand(ctx.hand),
            seen_onehot: self.encode_seen(ctx.round, ctx.tracker),
            trick_led_suit,
            trick_cards,
            trick_count,
            my_trick_position,
            trick_pad: 0.0,
            scores_relative: self.encode_scores_relative(ctx.scores, ctx.seat),
            hearts_broken: if ctx.round.hearts_broken() { 1.0 } else { 0.0 },
            tricks_completed: ctx.round.tricks_completed() as f32 / 13.0,
            passing_phase: if matches!(ctx.round.phase(), RoundPhase::Passing(_)) {
                1.0
            } else {
                0.0
            },
            passing_direction: self.encode_direction(ctx.passing_direction),
            opp_voids: self.encode_opponent_voids(ctx),
            last_4_cards: self.encode_history(ctx.round),
        }
    }

    fn encode_hand(&self, hand: &hearts_core::model::hand::Hand) -> [f32; 52] {
        let mut features = [0.0f32; 52];
        for card in hand.iter() {
            features[card.to_id() as usize] = 1.0;
        }
        features
    }

    #[allow(dead_code)]
    fn encode_seen(&self, _round: &RoundState, tracker: &UnseenTracker) -> [f32; 52] {
        let mut features = [0.0f32; 52];
        for id in 0..52 {
            if let Some(card) = Card::from_id(id) {
                if !tracker.is_unseen(card) {
                    features[id as usize] = 1.0;
                }
            }
        }
        features
    }

    #[allow(dead_code)]
    fn encode_trick(
        &self,
        round: &RoundState,
        my_seat: PlayerPosition,
    ) -> ([f32; 4], [[f32; 17]; 4], f32, f32) {
        let trick = round.current_trick();
        let mut led_suit = [0.0f32; 4];
        let mut cards = [[0.0f32; 17]; 4];

        if let Some(suit) = trick.lead_suit() {
            led_suit[suit as usize] = 1.0;
        }

        let plays = trick.plays();
        for (idx, play) in plays.iter().enumerate() {
            if idx < 4 {
                cards[idx] = self.encode_card(play.card);
            }
        }

        let count = plays.len() as f32 / 4.0;

        let my_position = plays
            .iter()
            .position(|p| p.position == my_seat)
            .map(|pos| pos as f32 / 4.0)
            .unwrap_or(count);

        (led_suit, cards, count, my_position)
    }

    fn encode_card(&self, card: Card) -> [f32; 17] {
        let mut features = [0.0f32; 17];
        // One-hot suit (4 features)
        features[card.suit as usize] = 1.0;
        // One-hot rank (13 features: 2-A)
        let rank_index = (card.rank.value() - 2) as usize;
        features[4 + rank_index] = 1.0;
        features
    }

    #[allow(dead_code)]
    fn encode_scores_relative(&self, scores: &ScoreBoard, my_seat: PlayerPosition) -> [f32; 4] {
        [
            scores.score(my_seat) as f32 / 100.0,
            scores.score(my_seat.next()) as f32 / 100.0,
            scores.score(my_seat.opposite()) as f32 / 100.0,
            scores.score(my_seat.previous()) as f32 / 100.0,
        ]
    }

    fn encode_direction(&self, direction: PassingDirection) -> [f32; 4] {
        let mut features = [0.0f32; 4];
        let index = match direction {
            PassingDirection::Left => 0,
            PassingDirection::Right => 1,
            PassingDirection::Across => 2,
            PassingDirection::Hold => 3,
        };
        features[index] = 1.0;
        features
    }

    #[allow(dead_code)]
    fn encode_opponent_voids(&self, ctx: &PolicyContext) -> [f32; 12] {
        let voids = ctx.tracker.infer_voids(ctx.seat, ctx.round);
        let mut flat = [0.0f32; 12];

        // Encode 3 opponents × 4 suits
        let opponents = [ctx.seat.next(), ctx.seat.opposite(), ctx.seat.previous()];
        for (opp_idx, &opp_seat) in opponents.iter().enumerate() {
            for suit in Suit::ALL {
                if voids[opp_seat.index()][suit as usize] {
                    flat[opp_idx * 4 + suit as usize] = 1.0;
                }
            }
        }

        flat
    }

    #[allow(dead_code)]
    fn encode_history(&self, round: &RoundState) -> [f32; 68] {
        let mut features = [0.0f32; 68];
        let history = round.trick_history();

        // Take last 4 cards from completed tricks
        let mut cards_to_encode: Vec<Card> = Vec::new();
        for trick in history.iter().rev() {
            if let Some(_winner) = trick.winner() {
                for play in trick.plays() {
                    cards_to_encode.push(play.card);
                    if cards_to_encode.len() >= 4 {
                        break;
                    }
                }
            }
            if cards_to_encode.len() >= 4 {
                break;
            }
        }

        // Encode in reverse order (most recent first)
        cards_to_encode.reverse();
        for (idx, &card) in cards_to_encode.iter().enumerate() {
            if idx < 4 {
                let card_features = self.encode_card(card);
                features[idx * 17..(idx + 1) * 17].copy_from_slice(&card_features);
            }
        }

        features
    }
}

impl Default for ObservationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::rank::Rank;

    #[test]
    fn observation_flattens_to_correct_size() {
        let obs = Observation {
            hand_onehot: [0.0; 52],
            seen_onehot: [0.0; 52],
            trick_led_suit: [0.0; 4],
            trick_cards: [[0.0; 17]; 4],
            trick_count: 0.0,
            my_trick_position: 0.0,
            trick_pad: 0.0,
            scores_relative: [0.0; 4],
            hearts_broken: 0.0,
            tricks_completed: 0.0,
            passing_phase: 0.0,
            passing_direction: [0.0; 4],
            opp_voids: [0.0; 12],
            last_4_cards: [0.0; 68],
        };

        let arr = obs.as_array();
        assert_eq!(arr.len(), 270);
    }

    #[test]
    fn hand_encoding_is_one_hot() {
        let builder = ObservationBuilder::new();
        let mut hand = Hand::new();
        hand.add(Card::new(Rank::Two, Suit::Clubs)); // ID 0
        hand.add(Card::new(Rank::Ace, Suit::Hearts)); // ID 51

        let features = builder.encode_hand(&hand);
        assert_eq!(features[0], 1.0);
        assert_eq!(features[51], 1.0);
        assert_eq!(features.iter().filter(|&&x| x > 0.0).count(), 2);
    }

    #[test]
    fn card_encoding_has_17_features() {
        let builder = ObservationBuilder::new();
        let card = Card::new(Rank::Queen, Suit::Spades);
        let features = builder.encode_card(card);

        assert_eq!(features.len(), 17);
        // Spades is suit index 2
        assert_eq!(features[2], 1.0);
        // Queen is rank value 12, so rank index 10 (12-2), at position 4+10=14
        assert_eq!(features[14], 1.0);
        assert_eq!(features.iter().filter(|&&x| x > 0.0).count(), 2);
    }

    #[test]
    fn direction_encoding_is_one_hot() {
        let builder = ObservationBuilder::new();
        let features = builder.encode_direction(PassingDirection::Left);
        assert_eq!(features[0], 1.0);
        assert_eq!(features.iter().filter(|&&x| x > 0.0).count(), 1);

        let features = builder.encode_direction(PassingDirection::Hold);
        assert_eq!(features[3], 1.0);
    }
}
