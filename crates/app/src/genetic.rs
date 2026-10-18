//! Order-preserving crossover, weighted selection, mutation, and elitism.
use crate::model::LayoutScore;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gene {
    pub part: usize,
    pub id: i64,
    pub rotation: u32,
}
#[derive(Clone)]
pub struct Candidate {
    pub genes: Vec<Gene>,
    pub score: Option<LayoutScore>,
}
pub struct Random(u64);
impl Default for Random {
    fn default() -> Self {
        Self(0x5eed)
    }
}
impl Random {
    pub fn next(&mut self, limit: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % limit.max(1) as u64) as usize
    }
    fn mutate(&mut self, genes: &mut [Gene], mutation: u32, rotations: u32) {
        for i in 0..genes.len() {
            if i + 1 < genes.len() && self.next(100) < mutation as usize {
                genes.swap(i, i + 1);
            }
            if self.next(100) < mutation as usize {
                genes[i].rotation = self.next(rotations as usize) as u32;
            }
        }
    }
    pub fn population(
        &mut self,
        genes: Vec<Gene>,
        size: u32,
        mutation: u32,
        rotations: u32,
    ) -> Vec<Candidate> {
        let mut population = vec![Candidate { genes, score: None }];
        while population.len() < size as usize {
            let mut genes = population[0].genes.clone();
            // Keep the area-sorted seed, but explore genuinely different orders
            // as well as nearby mutations from the very first generation.
            if population.len() % 3 == 1 {
                genes.reverse();
            } else if population.len() % 3 == 2 {
                for i in (1..genes.len()).rev() {
                    let j = self.next(i + 1);
                    genes.swap(i, j);
                }
            }
            self.mutate(&mut genes, mutation, rotations);
            population.push(Candidate { genes, score: None });
        }
        population
    }
    pub fn generation(&mut self, population: &mut Vec<Candidate>, mutation: u32, rotations: u32) {
        population.sort_by(|a, b| {
            a.score
                .unwrap_or(LayoutScore::WORST)
                .compare(b.score.unwrap_or(LayoutScore::WORST))
        });
        let size = population.len();
        let mut next = vec![population[0].clone()];
        while next.len() < size {
            let a = self.weighted(size);
            let mut b = self.weighted(size);
            if a == b {
                b = (b + 1) % size;
            }
            let count = population[a].genes.len();
            let cut = if count > 1 {
                1 + self.next(count - 1)
            } else {
                count
            };
            let mut genes = population[a].genes[..cut].to_vec();
            let present: HashSet<_> = genes.iter().map(|g| g.id).collect();
            genes.extend(
                population[b]
                    .genes
                    .iter()
                    .filter(|g| !present.contains(&g.id)),
            );
            self.mutate(&mut genes, mutation, rotations);
            next.push(Candidate { genes, score: None });
        }
        *population = next;
    }
    fn weighted(&mut self, size: usize) -> usize {
        let mut ticket = self.next(size * (size + 1) / 2);
        for rank in 0..size {
            let weight = size - rank;
            if ticket < weight {
                return rank;
            }
            ticket -= weight;
        }
        size - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generations_preserve_copies_and_the_best_candidate() {
        let genes: Vec<_> = (0..12)
            .map(|id| Gene {
                part: id as usize / 2,
                id,
                rotation: 0,
            })
            .collect();
        let mut random = Random::default();
        let mut population = random.population(genes.clone(), 20, 50, 7);
        population[0].score = Some(LayoutScore {
            placed: 12,
            sheets: 1,
            fitness: 10.,
        });
        for individual in population.iter_mut().skip(1) {
            individual.score = Some(LayoutScore {
                placed: 11,
                sheets: 1,
                fitness: 20.,
            });
        }
        for _ in 0..10 {
            random.generation(&mut population, 50, 7);
            assert_eq!(population.len(), 20);
            assert_eq!(population[0].genes, genes);
            for candidate in &population {
                let ids: HashSet<_> = candidate.genes.iter().map(|g| g.id).collect();
                assert_eq!(ids.len(), 12);
                assert!(candidate.genes.iter().all(|g| g.rotation < 7));
            }
        }
        assert!(population.iter().any(|p| p.genes != genes));
    }
}
