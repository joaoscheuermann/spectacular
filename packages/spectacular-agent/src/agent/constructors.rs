use super::{Agent, AgentConfig};
use crate::context::{ApproximateTokenCounter, TokenCounter};
use crate::store::Store;
use crate::tool::ToolStorage;
use spectacular_llms::LlmProvider;
use std::sync::{Arc, Mutex, RwLock};

impl<P> Agent<P, ApproximateTokenCounter>
where
    P: LlmProvider,
{
    /// Creates an agent using the default configuration and approximate token counter.
    pub fn new(provider: P) -> Self {
        Self::with_config(provider, AgentConfig::default())
    }

    /// Creates an agent using explicit configuration and the default in-memory store.
    pub fn with_config(provider: P, config: AgentConfig) -> Self {
        Self::with_config_and_store(provider, config, Store::default())
    }

    /// Creates an agent using explicit configuration and durable event store state.
    pub fn with_config_and_store(provider: P, config: AgentConfig, store: Store) -> Self {
        Agent::with_config_store_and_token_counter(provider, config, store, ApproximateTokenCounter)
    }
}

impl<P, C> Agent<P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Creates an agent with a custom token counter and default configuration.
    pub fn with_token_counter(provider: P, token_counter: C) -> Self {
        Self::with_config_and_token_counter(provider, AgentConfig::default(), token_counter)
    }

    /// Creates an agent with explicit configuration, store, and token counter dependencies.
    pub fn with_config_store_and_token_counter(
        provider: P,
        config: AgentConfig,
        store: Store,
        token_counter: C,
    ) -> Self {
        Self {
            provider,
            token_counter,
            queue: Arc::new(Default::default()),
            store: Mutex::new(store),
            active_run_control: Arc::new(Mutex::new(None)),
            tools: RwLock::new(ToolStorage::default()),
            config,
        }
    }
}
