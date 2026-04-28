//! T1.0.2.20 — Strategy Tauri commands.
//!
//! `get_strategy`, `set_strategy`, `update_strategy_params`.
//! Thin wrappers over [`SwitchEngine`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::switch::{SmartWeights, SwitchEngine, SwitchStrategy};

/// Managed state type for the switch engine.
pub type SwitchEngineHandle = Arc<SwitchEngine>;

/// Full config snapshot returned by `get_strategy`.
#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyResponse {
    pub strategy: SwitchStrategy,
    pub max_retries: usize,
    pub smart_weights: SmartWeights,
}

/// Return the current strategy and associated parameters.
#[tauri::command]
pub async fn get_strategy(
    engine: State<'_, SwitchEngineHandle>,
) -> Result<StrategyResponse, String> {
    let config = engine.config().await;
    Ok(StrategyResponse {
        strategy: config.strategy,
        max_retries: config.max_retries,
        smart_weights: config.smart_weights,
    })
}

/// Change the active switching strategy.
#[tauri::command]
pub async fn set_strategy(
    engine: State<'_, SwitchEngineHandle>,
    strategy: SwitchStrategy,
) -> Result<(), String> {
    engine.set_strategy(strategy).await;
    Ok(())
}

/// Parameters that can be updated independently of the strategy.
#[derive(Debug, Deserialize)]
pub struct StrategyParams {
    pub max_retries: Option<usize>,
    pub smart_weights: Option<SmartWeights>,
}

/// Update strategy parameters (max retries and/or smart weights).
#[tauri::command]
pub async fn update_strategy_params(
    engine: State<'_, SwitchEngineHandle>,
    params: StrategyParams,
) -> Result<(), String> {
    if let Some(max) = params.max_retries {
        engine.set_max_retries(max).await;
    }
    if let Some(weights) = params.smart_weights {
        engine.set_smart_weights(weights).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderManager;

    #[tokio::test]
    async fn default_strategy_is_priority() {
        let mgr = Arc::new(ProviderManager::new());
        let engine = Arc::new(SwitchEngine::new(mgr));
        let config = engine.config().await;
        assert_eq!(config.strategy, SwitchStrategy::Priority);
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn set_and_get_strategy_round_trips() {
        let mgr = Arc::new(ProviderManager::new());
        let engine = Arc::new(SwitchEngine::new(mgr));
        engine.set_strategy(SwitchStrategy::Smart).await;
        assert_eq!(engine.strategy().await, SwitchStrategy::Smart);
    }

    #[tokio::test]
    async fn update_max_retries() {
        let mgr = Arc::new(ProviderManager::new());
        let engine = Arc::new(SwitchEngine::new(mgr));
        engine.set_max_retries(5).await;
        assert_eq!(engine.config().await.max_retries, 5);
    }
}
