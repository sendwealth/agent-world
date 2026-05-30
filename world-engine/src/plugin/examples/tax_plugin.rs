//! Example: Custom Tax Plugin
//!
//! A plugin that applies a configurable tax rate on every transaction.
//! Demonstrates the `OnTransaction` hook with `InterceptTransactions` permission.

use crate::plugin::{
    HookResult, OnTransaction, PluginHooks, PluginManager, PluginMetadata, Permission,
    PermissionSet, TransactionContext,
};

/// A plugin that applies tax rules to transactions.
///
/// Configuration (via genesis.yaml):
/// ```yaml
/// plugins:
///   tax-plugin:
///     rate: 0.05        # 5% tax rate
///     threshold: 100    # Only tax transactions above 100 tokens
/// ```
pub struct TaxPlugin {
    /// Tax rate (0.0 - 1.0).
    pub rate: f64,
    /// Minimum transaction amount to trigger tax.
    pub threshold: u64,
}

impl TaxPlugin {
    pub fn new(rate: f64, threshold: u64) -> Self {
        Self { rate, threshold }
    }

    /// Default: 5% tax on transactions above 100 tokens.
    pub fn default_plugin() -> Self {
        Self {
            rate: 0.05,
            threshold: 100,
        }
    }

    /// Register this plugin with the given manager.
    pub fn register(self, manager: &mut PluginManager) -> Result<(), crate::plugin::PluginError> {
        let metadata = PluginMetadata {
            id: "com.agent-world.tax-plugin".to_string(),
            name: "Transaction Tax Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Applies a configurable tax rate on transactions above a threshold."
                .to_string(),
            author: "Agent World Team".to_string(),
            priority: 50,
        };

        let permissions = PermissionSet::from_permissions([
            Permission::ReadWorldState,
            Permission::InterceptTransactions,
            Permission::ReadEvents,
        ]);

        let hooks = PluginHooks {
            on_transaction: Some(Box::new(self)),
            ..Default::default()
        };

        manager.register_plugin(metadata, permissions, hooks)
    }
}

impl OnTransaction for TaxPlugin {
    fn on_transaction(&self, ctx: &TransactionContext) -> HookResult {
        if ctx.amount > self.threshold {
            let tax = (ctx.amount as f64 * self.rate) as u64;
            tracing::info!(
                "[TaxPlugin] Transaction {} -> {}: amount={}, tax={}",
                ctx.from,
                ctx.to,
                ctx.amount,
                tax
            );
            // In a real implementation, this would modify the transaction
            // by deducting tax from the sender and/or adding it to a treasury.
            // For now, we just log and allow the transaction.
        }
        HookResult::Continue
    }
}
