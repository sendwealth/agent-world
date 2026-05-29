//! Hook traits — optional interfaces plugins can implement.
//!
//! Each hook trait corresponds to an extension point in the engine.
//! Plugins implement only the hooks they need.

use super::context::{
    AgentActionContext, AgentSpawnContext, EventContext, ShutdownContext, StartupContext,
    TickContext, TransactionContext,
};

/// Result of a hook invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult {
    /// Continue processing (default).
    Continue,
    /// Skip the remaining hooks in this phase.
    SkipRemaining,
    /// Block the action entirely (only valid for intercept hooks).
    Block(String),
}

/// Hook: called before the tick subsystems execute.
///
/// Requires `ReadWorldState` permission.
pub trait OnTickStart: Send + Sync {
    fn on_tick_start(&self, ctx: &TickContext) -> HookResult {
        let _ = ctx;
        HookResult::Continue
    }
}

/// Hook: called after all tick subsystems have executed.
///
/// Requires `ReadWorldState` permission.
pub trait OnTickEnd: Send + Sync {
    fn on_tick_end(&self, ctx: &TickContext) -> HookResult {
        let _ = ctx;
        HookResult::Continue
    }
}

/// Hook: called before an agent action is processed.
///
/// Requires `InterceptActions` permission.
/// Returning `HookResult::Block(reason)` prevents the action.
pub trait OnAgentAction: Send + Sync {
    fn on_agent_action(&self, ctx: &AgentActionContext) -> HookResult {
        let _ = ctx;
        HookResult::Continue
    }
}

/// Hook: called before a transaction is committed.
///
/// Requires `InterceptTransactions` permission.
/// Returning `HookResult::Block(reason)` prevents the transaction.
pub trait OnTransaction: Send + Sync {
    fn on_transaction(&self, ctx: &TransactionContext) -> HookResult {
        let _ = ctx;
        HookResult::Continue
    }
}

/// Hook: called when an agent is spawned.
///
/// Requires `ReadAgents` permission.
pub trait OnAgentSpawn: Send + Sync {
    fn on_agent_spawn(&self, ctx: &AgentSpawnContext) -> HookResult {
        let _ = ctx;
        HookResult::Continue
    }
}

/// Hook: called for every event emitted.
///
/// Requires `ReadEvents` permission.
/// This is a read-only notification hook — `Block` is not honored.
pub trait OnEvent: Send + Sync {
    fn on_event(&self, ctx: &EventContext) -> HookResult {
        let _ = ctx;
        HookResult::Continue
    }
}

/// Hook: called once after all plugins have been loaded and initialized.
pub trait OnStartup: Send + Sync {
    fn on_startup(&self, ctx: &StartupContext) {
        let _ = ctx;
    }
}

/// Hook: called once before the engine shuts down.
pub trait OnShutdown: Send + Sync {
    fn on_shutdown(&self, ctx: &ShutdownContext) {
        let _ = ctx;
    }
}
