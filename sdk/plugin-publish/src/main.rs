//! plugin-publish — Publish Agent World plugins to the skill marketplace.
//!
//! Usage:
//!   plugin-publish my_plugin.awp
//!   plugin-publish my_plugin.awp --registry https://marketplace.agentworld.dev
//!   plugin-publish my_plugin.awp --dry-run

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;
use colored::*;

#[derive(Parser, Debug)]
#[command(name = "plugin-publish")]
#[command(about = "Publish an Agent World plugin (.awp) to the skill marketplace")]
#[command(version)]
struct Cli {
    /// Path to the .awp bundle file
    #[arg(value_name = "BUNDLE")]
    bundle: PathBuf,

    /// Marketplace registry URL
    #[arg(short, long, default_value = "https://marketplace.agentworld.dev")]
    registry: String,

    /// API key for authentication (or set AGENT_WORLD_API_KEY env var)
    #[arg(short, long, env = "AGENT_WORLD_API_KEY")]
    api_key: Option<String>,

    /// Dry run — validate without uploading
    #[arg(long)]
    dry_run: bool,

    /// Overwrite existing version
    #[arg(long)]
    force: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate bundle
    if !cli.bundle.exists() {
        bail!("Bundle not found: {}", cli.bundle.display());
    }

    if !cli.bundle.extension().map(|e| e == "awp").unwrap_or(false) {
        bail!("Expected a .awp bundle file, got: {}", cli.bundle.display());
    }

    // Read and validate bundle
    println!("{} Loading bundle: {}", "📦".bold(), cli.bundle.display());
    let bundle_data = std::fs::read(&cli.bundle)
        .with_context(|| format!("Cannot read bundle: {}", cli.bundle.display()))?;

    let manifest = validate_bundle(&bundle_data, &cli.bundle)?;

    println!("  Plugin: {} v{}", manifest.plugin_id.bold(), manifest.version);
    println!("  WASM: {} bytes", manifest.wasm_size.to_string().green());
    println!("  Hash: {}…{}", &manifest.wasm_hash[..12].dimmed(), &manifest.wasm_hash[56..].dimmed());

    if cli.dry_run {
        println!("{} Dry run — bundle is valid, skipping upload.", "🔍".yellow().bold());
        return Ok(());
    }

    // Check API key
    let api_key = match &cli.api_key {
        Some(key) => key.clone(),
        None => bail!("API key required. Set --api-key or AGENT_WORLD_API_KEY env var."),
    };

    // Upload to registry
    let url = format!("{}/api/v1/plugins", cli.registry.trim_end_matches('/'));
    println!("{} Publishing to: {}", "📤".bold(), url);

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/octet-stream")
        .query(&[("force", cli.force)])
        .body(bundle_data)
        .send()
        .await
        .with_context(|| "Failed to connect to marketplace registry")?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        let plugin_url = body["url"].as_str().unwrap_or("unknown");
        println!("{} Published successfully! 🎉", "✅".green().bold());
        println!("  URL: {}", plugin_url.bold());
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("Publish failed ({}): {}", status, body);
    }

    Ok(())
}

#[derive(Debug)]
struct BundleManifest {
    plugin_id: String,
    version: String,
    wasm_size: u64,
    wasm_hash: String,
}

fn validate_bundle(data: &[u8], path: &PathBuf) -> Result<BundleManifest> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .with_context(|| format!("Invalid .awp bundle: {}", path.display()))?;

    // Read bundle.json
    let mut bundle_file = archive.by_name("bundle.json")
        .with_context(|| "Bundle missing bundle.json")?;
    let mut bundle_json = String::new();
    std::io::Read::read_to_end(&mut bundle_file, &mut bundle_json.as_bytes().to_vec().as_mut_slice())
        .ok();
    // Use read_to_string instead
    let mut bundle_file = archive.by_name("bundle.json")?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut bundle_file, &mut buf)?;
    let bundle_json_str = String::from_utf8(buf)?;

    let bundle_value: serde_json::Value = serde_json::from_str(&bundle_json_str)?;

    // Verify WASM hash
    let expected_hash = bundle_value["wasm_hash"].as_str().unwrap_or("");
    let mut wasm_file = archive.by_name("plugin.wasm")?;
    let mut wasm_bytes = Vec::new();
    std::io::Read::read_to_end(&mut wasm_file, &mut wasm_bytes)?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&wasm_bytes);
    let actual_hash = format!("{:x}", hasher.finalize());

    if actual_hash != expected_hash {
        bail!("WASM hash mismatch in bundle! Expected {}, got {}", expected_hash, actual_hash);
    }

    Ok(BundleManifest {
        plugin_id: bundle_value["plugin_id"].as_str().unwrap_or("unknown").to_string(),
        version: bundle_value["version"].as_str().unwrap_or("0.0.0").to_string(),
        wasm_size: bundle_value["wasm_size"].as_u64().unwrap_or(0),
        wasm_hash: expected_hash.to_string(),
    })
}

use std::path::Path;
