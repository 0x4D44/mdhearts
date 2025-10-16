/// Generates deterministic seat permutations for 4-player games.
pub struct SeatPermutations {
    permutations: Vec<[usize; 4]>,
}

impl SeatPermutations {
    pub fn new(count: usize) -> Self {
        let limit = count.min(24);
        let mut permutations = Vec::with_capacity(limit);
        let mut base = [0usize, 1, 2, 3];
        generate(&mut base, 0, limit, &mut permutations);
        Self { permutations }
    }

    pub fn as_slice(&self) -> &[[usize; 4]] {
        &self.permutations
    }
}

fn generate(data: &mut [usize; 4], start: usize, limit: usize, output: &mut Vec<[usize; 4]>) {
    if output.len() >= limit {
        return;
    }

    if start == data.len() - 1 {
        output.push(*data);
        return;
    }

    for idx in start..data.len() {
        data.swap(start, idx);
        generate(data, start + 1, limit, output);
        data.swap(start, idx);
        if output.len() >= limit {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_expected_first_permutation() {
        let perms = SeatPermutations::new(1);
        assert_eq!(perms.as_slice(), &[[0, 1, 2, 3]]);
    }

    #[test]
    fn caps_at_twenty_four() {
        let perms = SeatPermutations::new(100);
        assert_eq!(perms.as_slice().len(), 24);
    }
}
