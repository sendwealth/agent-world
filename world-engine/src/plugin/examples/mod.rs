//! Example plugins demonstrating the plugin API.
//!
//! These are built-in example plugins that show how to:
//! - Implement hook traits
//! - Register with the PluginManager
//! - Use permissions

mod data_collector;
mod tax_plugin;

pub use data_collector::DataCollectorPlugin;
pub use tax_plugin::TaxPlugin;
