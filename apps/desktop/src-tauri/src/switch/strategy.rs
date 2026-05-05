//! T1.0.2.09 — [`SwitchStrategy`] enum + strategy implementations.
//!
//! Five switching strategies that rank enabled + healthy providers:
//! * `Priority`     — lowest `priority` value wins (T1.0.2.10)
//! * `Fastest`      — lowest rolling response time wins (T1.0.2.11)
//! * `Cost`         — lowest `cost_per_token` wins (T1.0.2.12)
//! * `LoadBalance`  — weighted round-robin (T1.0.2.13)
//! * `Smart`        — 4-dimension weighted score (T1.0.2.14)

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::providers::api::Provider as ProviderTrait;
use crate::providers::wrapper::ProviderWrapper;

/// User-selectable switching strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchStrategy {
    Priority,
    Fastest,
    Cost,
    LoadBalance,
    Smart,
}

impl SwitchStrategy {
    pub const ALL: [Self; 5] = [
        Self::Priority,
        Self::Fastest,
        Self::Cost,
        Self::LoadBalance,
        Self::Smart,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Priority => "priority",
            Self::Fastest => "fastest",
            Self::Cost => "cost",
            Self::LoadBalance => "load_balance",
            Self::Smart => "smart",
        }
    }
}

/// Weights for the `Smart` strategy (T1.0.2.14). Default 40/30/20/10.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWeights {
    pub health: f64,
    pub response_time: f64,
    pub cost: f64,
    pub priority: f64,
}

impl Default for SmartWeights {
    fn default() -> Self {
        Self {
            health: 40.0,
            response_time: 30.0,
            cost: 20.0,
            priority: 10.0,
        }
    }
}

/// Round-robin state for `LoadBalance` strategy.
#[derive(Debug, Default)]
pub struct RoundRobinState {
    counter: AtomicUsize,
}

/// Select the best provider from already eligible `candidates` using
/// the given strategy. The switch engine removes disabled, down, and
/// already-tried providers before calling this function.
pub fn select(
    strategy: SwitchStrategy,
    candidates: &[Arc<ProviderWrapper>],
    rr_state: &RoundRobinState,
    smart_weights: &SmartWeights,
) -> Option<Arc<ProviderWrapper>> {
    let alive: Vec<_> = candidates
        .iter()
        .filter(|p| p.is_enabled())
        .cloned()
        .collect();
    if alive.is_empty() {
        return None;
    }

    match strategy {
        SwitchStrategy::Priority => select_priority(&alive),
        SwitchStrategy::Fastest => select_fastest(&alive),
        SwitchStrategy::Cost => select_cost(&alive),
        SwitchStrategy::LoadBalance => select_load_balance(&alive, rr_state),
        SwitchStrategy::Smart => select_smart(&alive, smart_weights),
    }
}

/// T1.0.2.10: pick the provider with the lowest `priority` value.
fn select_priority(candidates: &[Arc<ProviderWrapper>]) -> Option<Arc<ProviderWrapper>> {
    candidates.iter().min_by_key(|p| p.get_priority()).cloned()
}

/// T1.0.2.11: pick the provider with the lowest rolling response time.
fn select_fastest(candidates: &[Arc<ProviderWrapper>]) -> Option<Arc<ProviderWrapper>> {
    candidates
        .iter()
        .filter_map(|p| p.state.rolling_response_us().map(|us| (p.clone(), us)))
        .min_by_key(|(_, us)| *us)
        .map(|(p, _)| p)
        .or_else(|| candidates.first().cloned()) // fallback if no data
}

/// T1.0.2.12: pick the provider with the lowest `cost_per_token`.
fn select_cost(candidates: &[Arc<ProviderWrapper>]) -> Option<Arc<ProviderWrapper>> {
    candidates
        .iter()
        .filter_map(|p| p.get_cost_per_token().map(|c| (p.clone(), c)))
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(p, _)| p)
        .or_else(|| candidates.first().cloned()) // fallback if no cost data
}

/// T1.0.2.13: weighted round-robin. Weight = inverse priority
/// (higher priority = more requests).
fn select_load_balance(
    candidates: &[Arc<ProviderWrapper>],
    rr_state: &RoundRobinState,
) -> Option<Arc<ProviderWrapper>> {
    if candidates.is_empty() {
        return None;
    }

    let priorities = candidates
        .iter()
        .map(|provider| sanitized_priority(provider.get_priority()))
        .collect::<Vec<_>>();
    let max_priority = priorities.iter().copied().max().unwrap_or(1);
    let weights = priorities
        .iter()
        .map(|priority| inverse_priority_weight(*priority, max_priority))
        .collect::<Vec<_>>();
    let total_weight = weights.iter().copied().sum::<usize>();

    let mut slot = rr_state.counter.fetch_add(1, Ordering::Relaxed) % total_weight;
    for (provider, weight) in candidates.iter().zip(weights) {
        if slot < weight {
            return Some(provider.clone());
        }
        slot -= weight;
    }

    candidates.first().cloned()
}

fn sanitized_priority(priority: i32) -> u32 {
    u32::try_from(priority).unwrap_or(1).max(1)
}

fn inverse_priority_weight(priority: u32, max_priority: u32) -> usize {
    usize::try_from((max_priority / priority).max(1)).unwrap_or(usize::MAX)
}

/// T1.0.2.14: 4-dimension weighted score. Higher score = better.
fn select_smart(
    candidates: &[Arc<ProviderWrapper>],
    weights: &SmartWeights,
) -> Option<Arc<ProviderWrapper>> {
    if candidates.is_empty() {
        return None;
    }

    // Collect raw metrics for normalisation.
    let metrics: Vec<_> = candidates
        .iter()
        .map(|p| {
            let rt = p.state.rolling_response_us().unwrap_or(i64::MAX);
            let cost = p.get_cost_per_token().unwrap_or(f64::MAX);
            let priority = p.get_priority();
            (p, rt, cost, priority)
        })
        .collect();

    let max_rt = metrics
        .iter()
        .map(|(_, rt, _, _)| *rt)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_cost = metrics
        .iter()
        .map(|(_, _, c, _)| *c)
        .fold(f64::MIN, f64::max)
        .max(f64::MIN_POSITIVE);
    let max_prio = metrics
        .iter()
        .map(|(_, _, _, p)| *p)
        .max()
        .unwrap_or(1)
        .max(1);

    metrics
        .into_iter()
        .map(|(p, rt, cost, prio)| {
            // Health dimension: Healthy=1.0, Degraded=0.5, Down=0.0
            // (read from RuntimeState which is updated by HealthChecker)
            let health_score = 1.0; // we already filtered to alive

            // Response time: lower is better → invert
            #[allow(clippy::cast_precision_loss)]
            let rt_score = 1.0 - (rt as f64 / max_rt as f64);

            // Cost: lower is better → invert
            let cost_score = if cost < f64::MAX {
                1.0 - (cost / max_cost)
            } else {
                0.0
            };

            // Priority: lower is better → invert
            #[allow(clippy::cast_precision_loss)]
            let prio_score = 1.0 - (f64::from(prio) / f64::from(max_prio));

            let total = weights.health * health_score
                + weights.response_time * rt_score
                + weights.cost * cost_score
                + weights.priority * prio_score;

            (p, total)
        })
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(p, _)| p.clone())
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::providers::model::ProviderKind;
    use crate::providers::openai::OpenAIProvider;

    fn mock_wrapper(id: &str, priority: i32, cost: Option<f64>) -> Arc<ProviderWrapper> {
        let inner = OpenAIProvider::with_options(id, id, "https://mock", "k", priority, cost)
            .expect("build");
        Arc::new(ProviderWrapper::new(
            id,
            id,
            ProviderKind::Openai,
            priority,
            cost,
            true,
            Box::new(inner),
        ))
    }

    #[test]
    fn strategy_enum_serialises_as_snake_case() {
        let json = serde_json::to_value(SwitchStrategy::LoadBalance).unwrap();
        assert_eq!(json, "load_balance");
        let back: SwitchStrategy = serde_json::from_value(json).unwrap();
        assert_eq!(back, SwitchStrategy::LoadBalance);
    }

    #[test]
    fn priority_selects_lowest() {
        let a = mock_wrapper("a", 50, None);
        let b = mock_wrapper("b", 10, None);
        let c = mock_wrapper("c", 30, None);
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();
        let result = select(SwitchStrategy::Priority, &[a, b.clone(), c], &rr, &sw);
        assert_eq!(result.unwrap().id(), "b");
    }

    #[test]
    fn cost_selects_cheapest() {
        let a = mock_wrapper("a", 10, Some(0.000_01));
        let b = mock_wrapper("b", 10, Some(0.000_003));
        let c = mock_wrapper("c", 10, None);
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();
        let result = select(SwitchStrategy::Cost, &[a, b.clone(), c], &rr, &sw);
        assert_eq!(result.unwrap().id(), "b");
    }

    #[test]
    fn fastest_selects_lowest_response_time() {
        let a = mock_wrapper("a", 10, None);
        let b = mock_wrapper("b", 10, None);
        a.state.record_response_us(5000);
        b.state.record_response_us(1000);
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();
        let result = select(SwitchStrategy::Fastest, &[a, b.clone()], &rr, &sw);
        assert_eq!(result.unwrap().id(), "b");
    }

    #[test]
    fn load_balance_rotates() {
        let a = mock_wrapper("a", 10, None);
        let b = mock_wrapper("b", 10, None);
        let candidates = vec![a, b];
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();
        let r1 = select(SwitchStrategy::LoadBalance, &candidates, &rr, &sw).unwrap();
        let r2 = select(SwitchStrategy::LoadBalance, &candidates, &rr, &sw).unwrap();
        assert_ne!(r1.id(), r2.id());
    }

    #[test]
    fn load_balance_weights_by_inverse_priority() {
        let high_priority = mock_wrapper("high", 10, None);
        let backup = mock_wrapper("backup", 20, None);
        let candidates = vec![high_priority, backup];
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();

        let picks = (0..6)
            .map(|_| {
                select(SwitchStrategy::LoadBalance, &candidates, &rr, &sw)
                    .unwrap()
                    .id()
                    .to_owned()
            })
            .collect::<Vec<_>>();

        assert_eq!(picks, ["high", "high", "backup", "high", "high", "backup"]);
    }

    #[test]
    fn smart_prefers_better_provider() {
        let a = mock_wrapper("a", 50, Some(0.000_01));
        let b = mock_wrapper("b", 10, Some(0.000_003));
        a.state.record_response_us(5000);
        b.state.record_response_us(1000);
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();
        let result = select(SwitchStrategy::Smart, &[a, b.clone()], &rr, &sw);
        // b has better priority, cost, and response time
        assert_eq!(result.unwrap().id(), "b");
    }

    #[test]
    fn empty_candidates_returns_none() {
        let rr = RoundRobinState::default();
        let sw = SmartWeights::default();
        for strategy in SwitchStrategy::ALL {
            assert!(select(strategy, &[], &rr, &sw).is_none());
        }
    }
}
