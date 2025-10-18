//! Simple cache for sampled worlds keyed by coarse belief descriptors.

use super::{Belief, BeliefUpdateCtx, SampledWorld, SuitCounts, SuitMask};
use crate::model::player::PlayerPosition;
use crate::model::suit::Suit;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BeliefCacheKey {
    trick_index: u8,
    lead_suit: Option<Suit>,
    voids: [SuitMask; 4],
    remain: SuitCounts,
}

impl BeliefCacheKey {
    pub fn new(
        trick_index: u8,
        lead_suit: Option<Suit>,
        voids: [SuitMask; 4],
        remain: SuitCounts,
    ) -> Self {
        Self {
            trick_index,
            lead_suit,
            voids,
            remain,
        }
    }

    pub fn from_belief(belief: &Belief, ctx: &BeliefUpdateCtx) -> Self {
        let mut voids = [SuitMask::EMPTY; 4];
        for seat in PlayerPosition::LOOP {
            voids[seat.index()] = belief.void_mask(seat);
        }
        Self::new(
            belief.trick_index(),
            ctx.lead_suit(),
            voids,
            belief.remaining_suits(),
        )
    }
}

/// Stores sampled worlds with an LRU eviction policy.
#[derive(Debug)]
pub struct SamplerCache {
    entries: HashMap<BeliefCacheKey, Vec<SampledWorld>>,
    order: VecDeque<BeliefCacheKey>,
    capacity: usize,
}

impl SamplerCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn get(&self, key: &BeliefCacheKey) -> Option<&[SampledWorld]> {
        self.entries.get(key).map(|worlds| worlds.as_slice())
    }

    pub fn insert(&mut self, key: BeliefCacheKey, world: SampledWorld) {
        if self.capacity == 0 {
            return;
        }
        let entry = self.entries.entry(key.clone()).or_default();
        entry.push(world);
        if !self.order.contains(&key) {
            self.order.push_back(key.clone());
        }
        self.evict_if_needed();
    }

    fn evict_if_needed(&mut self) {
        while self.capacity > 0 && self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::model::hand::Hand;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::suit::Suit;
    use std::array;

    #[test]
    fn cache_respects_capacity() {
        let mut cache = SamplerCache::new(1);
        let key_a = BeliefCacheKey::new(
            0,
            Some(Suit::Clubs),
            [SuitMask::EMPTY; 4],
            SuitCounts::new_full_deck(),
        );
        let key_b = BeliefCacheKey::new(
            1,
            Some(Suit::Spades),
            [SuitMask::EMPTY; 4],
            SuitCounts::new_full_deck(),
        );
        cache.insert(key_a.clone(), dummy_world());
        cache.insert(key_b.clone(), dummy_world());
        assert!(cache.get(&key_a).is_none());
        assert!(cache.get(&key_b).is_some());
    }

    fn dummy_world() -> SampledWorld {
        let mut hands = array::from_fn(|_| Hand::new());
        hands[PlayerPosition::North.index()].add(Card::new(Rank::Two, Suit::Clubs));
        SampledWorld::new(hands, 0.0)
    }
}
