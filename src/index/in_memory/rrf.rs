//! Reciprocal Rank Fusion for combining multiple ranked lists.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::common::{Buildable, Configurable, VaultPath};

/// Configuration for Reciprocal Rank Fusion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RRFConfig {
    /// RRF constant `k` (default: 60.0).
    ///
    /// Higher values reduce the influence of high ranks from individual pipelines.
    #[serde(default = "default_k")]
    pub k: f32,
}

fn default_k() -> f32 {
    60.0
}

impl Default for RRFConfig {
    fn default() -> Self {
        Self { k: default_k() }
    }
}

impl Buildable for RRFConfig {
    type Target = RRF;
    type Error = std::convert::Infallible;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        Ok(RRF { k: self.k })
    }
}

/// Reciprocal Rank Fusion combiner.
///
/// Fuses multiple ranked lists into a single ranking using:
/// `RRF_score(d) = Σ 1/(k + rank)` where rank is 1-based position
/// in each pipeline.
#[derive(Debug, Clone)]
pub struct RRF {
    k: f32,
}

impl RRF {
    /// Fuse multiple ranked lists into a single ranking.
    ///
    /// Each pipeline is a `Vec<(VaultPath, f32)>` sorted by score descending.
    /// Returns fused results sorted by RRF score descending, truncated to `limit`.
    pub fn fuse(&self, pipelines: &[Vec<(VaultPath, f32)>], limit: usize) -> Vec<(VaultPath, f32)> {
        let mut scores: HashMap<VaultPath, f32> = HashMap::new();

        for pipeline in pipelines {
            for (rank_0, (path, _score)) in pipeline.iter().enumerate() {
                let rank = (rank_0 + 1) as f32; // 1-based
                *scores.entry(path.clone()).or_default() += 1.0 / (self.k + rank);
            }
        }

        let mut results: Vec<_> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }
}

impl Configurable for RRF {
    type Config = RRFConfig;

    fn config(&self) -> Self::Config {
        RRFConfig { k: self.k }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> VaultPath {
        VaultPath::new(s).unwrap()
    }

    #[test]
    fn single_pipeline() {
        let rrf = RRF { k: 60.0 };
        let pipeline = vec![
            (path("a.md#"), 10.0),
            (path("b.md#"), 5.0),
            (path("c.md#"), 1.0),
        ];

        let results = rrf.fuse(&[pipeline], 10);
        assert_eq!(results.len(), 3);
        // Order preserved: a, b, c
        assert_eq!(results[0].0, path("a.md#"));
        assert_eq!(results[1].0, path("b.md#"));
        assert_eq!(results[2].0, path("c.md#"));
    }

    #[test]
    fn two_pipelines_overlapping() {
        let rrf = RRF { k: 60.0 };
        let p1 = vec![(path("a.md#"), 10.0), (path("b.md#"), 5.0)];
        let p2 = vec![(path("b.md#"), 8.0), (path("a.md#"), 3.0)];

        let results = rrf.fuse(&[p1, p2], 10);
        assert_eq!(results.len(), 2);
        // Both a and b appear in both pipelines
        // a: 1/(60+1) + 1/(60+2) = ~0.01639 + ~0.01613 = ~0.03252
        // b: 1/(60+2) + 1/(60+1) = ~0.01613 + ~0.01639 = ~0.03252
        // Scores are equal (symmetric), order may vary
        let a_score = results.iter().find(|(p, _)| *p == path("a.md#")).unwrap().1;
        let b_score = results.iter().find(|(p, _)| *p == path("b.md#")).unwrap().1;
        assert!((a_score - b_score).abs() < 0.001);
    }

    #[test]
    fn two_pipelines_disjoint() {
        let rrf = RRF { k: 60.0 };
        let p1 = vec![(path("a.md#"), 10.0)];
        let p2 = vec![(path("b.md#"), 8.0)];

        let results = rrf.fuse(&[p1, p2], 10);
        assert_eq!(results.len(), 2);
        // Both have same RRF score: 1/(60+1)
        let a_score = results.iter().find(|(p, _)| *p == path("a.md#")).unwrap().1;
        let b_score = results.iter().find(|(p, _)| *p == path("b.md#")).unwrap().1;
        assert!((a_score - b_score).abs() < f32::EPSILON);
    }

    #[test]
    fn limit_truncates() {
        let rrf = RRF { k: 60.0 };
        let pipeline = vec![
            (path("a.md#"), 10.0),
            (path("b.md#"), 5.0),
            (path("c.md#"), 1.0),
        ];

        let results = rrf.fuse(&[pipeline], 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn empty_pipelines() {
        let rrf = RRF { k: 60.0 };
        let results = rrf.fuse(&[], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn boost_from_multiple_pipelines() {
        let rrf = RRF { k: 60.0 };
        // doc "a" appears rank 1 in both pipelines, "b" only in one
        let p1 = vec![(path("a.md#"), 10.0), (path("b.md#"), 5.0)];
        let p2 = vec![(path("a.md#"), 8.0)];

        let results = rrf.fuse(&[p1, p2], 10);
        assert_eq!(results[0].0, path("a.md#")); // boosted by appearing in both
    }

    #[test]
    fn config_roundtrip() {
        let config = RRFConfig { k: 42.0 };
        let rrf = config.build().unwrap();
        assert_eq!(rrf.config(), RRFConfig { k: 42.0 });
    }

    #[test]
    fn default_config() {
        let config = RRFConfig::default();
        assert!((config.k - 60.0).abs() < f32::EPSILON);
    }
}
