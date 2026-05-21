//! plugin-pack — Compile, pack, and verify Agent World plugins.
//!
//! Usage:
//!   plugin-pack ./target/wasm32-unknown-unknown/release/my_plugin.wasm
//!   plugin-pack ./target/wasm32-unknown-unknown/release/my_plugin.wasm --manifest skills.yaml
//!   plugin-pack ./target/wasm32-unknown-unknown/release/my_plugin.wasm --output my_plugin.awp

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use colored::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(name = "plugin-pack")]
#[command(about = "Pack an Agent World plugin WASM into a distributable .awp bundle")]
#[command(version)]
struct Cli {
    /// Path to the compiled WASM file
    #[arg(value_name = "WASM_FILE")]
    wasm: PathBuf,

    /// Path to skills.yaml manifest
    #[arg(short, long, default_value = "skills.yaml")]
    manifest: PathBuf,

    /// Output path (default: <plugin-id>.awp)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Skip validation
    #[arg(long)]
    skip_validation: bool,
}

/// Plugin bundle manifest (embedded in .awp archive).
#[derive(Debug, Serialize, Deserialize)]
struct BundleManifest {
    /// Plugin ID from skills.yaml.
    plugin_id: String,
    /// Plugin version.
    version: String,
    /// SHA-256 hash of the WASM binary.
    wasm_hash: String,
    /// WASM file size in bytes.
    wasm_size: u64,
    /// Timestamp of packaging.
    packaged_at: String,
    /// Pack tool version.
    pack_version: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate WASM file
    if !cli.wasm.exists() {
        bail!("WASM file not found: {}", cli.wasm.display());
    }

    let wasm_bytes = std::fs::read(&cli.wasm)
        .with_context(|| format!("Cannot read WASM file: {}", cli.wasm.display()))?;

    if !cli.skip_validation {
        validate_wasm(&wasm_bytes, &cli.wasm)?;
    }

    // Parse manifest
    if !cli.manifest.exists() {
        bail!("Manifest not found: {}", cli.manifest.display());
    }

    let manifest_content = std::fs::read_to_string(&cli.manifest)?;
    let manifest: serde_yaml::Value = serde_yaml::from_str(&manifest_content)
        .with_context(|| "Invalid skills.yaml")?;

    let plugin_id = manifest["plugin"]["id"]
        .as_str()
        .unwrap_or("unknown/plugin")
        .to_string();
    let version = manifest["plugin"]["version"]
        .as_str()
        .unwrap_or("0.0.0")
        .to_string();

    println!("{} Packaging plugin: {} v{}", "🔌".bold(), plugin_id.bold(), version);

    // Compute WASM hash
    let mut hasher = Sha256::new();
    hasher.update(&wasm_bytes);
    let hash = format!("{:x}", hasher.finalize());

    println!("  {} WASM: {} bytes, SHA-256: {}…{}",
             "📦".to_string().dimmed(),
             wasm_bytes.len().to_string().green(),
             &hash[..12].dimmed(),
             &hash[56..].dimmed());

    // Create bundle manifest
    let bundle_manifest = BundleManifest {
        plugin_id: plugin_id.clone(),
        version: version.clone(),
        wasm_hash: hash,
        wasm_size: wasm_bytes.len() as u64,
        packaged_at: chrono_now_or_fallback(),
        pack_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    // Determine output path
    let output_path = cli.output.unwrap_or_else(|| {
        let safe_name = plugin_id.replace('/', "_");
        PathBuf::from(format!("{}.awp", safe_name))
    });

    // Create .awp bundle (ZIP format)
    let file = std::fs::File::create(&output_path)
        .with_context(|| format!("Cannot create output file: {}", output_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("plugin.wasm", options)?;
    zip.write_all(&wasm_bytes)?;

    zip.start_file("skills.yaml", options)?;
    zip.write_all(manifest_content.as_bytes())?;

    zip.start_file("bundle.json", options)?;
    let bundle_json = serde_json::to_string_pretty(&bundle_manifest)?;
    zip.write_all(bundle_json.as_bytes())?;

    zip.finish()?;

    let output_size = std::fs::metadata(&output_path)?.len();

    println!("  {} Written: {} ({} bytes)",
             "✅".green(),
             output_path.display().to_string().bold(),
             output_size.to_string().green());

    // Verify bundle
    println!("  {} Verifying bundle integrity…", "🔍".to_string().dimmed());
    verify_bundle(&output_path, &bundle_manifest)?;

    println!("{} Plugin packed successfully! 🎉", "✅".green().bold());

    Ok(())
}

fn validate_wasm(bytes: &[u8], path: &Path) -> Result<()> {
    if bytes.len() < 8 {
        bail!("File too small to be valid WASM: {}", path.display());
    }

    // Check magic bytes: \x00asm
    if &bytes[0..4] != b"\x00asm" {
        bail!(
            "Invalid WASM magic bytes in {}. Expected [0x00, 'a', 's', 'm']",
            path.display()
        );
    }

    // Check version (1)
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != 1 {
        bail!("Unsupported WASM version {} in {}", version, path.display());
    }

    println!("  {} Valid WASM module (version 1)", "✓".green());
    Ok(())
}

fn verify_bundle(path: &Path, expected: &BundleManifest) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Verify all expected files exist
    for name in &["plugin.wasm", "skills.yaml", "bundle.json"] {
        if archive.by_name(name).is_err() {
            bail!("Bundle missing file: {}", name);
        }
    }

    // Verify WASM hash
    let mut wasm_file = archive.by_name("plugin.wasm")?;
    let mut wasm_bytes = Vec::new();
    std::io::Read::read_to_end(&mut wasm_file, &mut wasm_bytes)?;

    let mut hasher = Sha256::new();
    hasher.update(&wasm_bytes);
    let actual_hash = format!("{:x}", hasher.finalize());

    if actual_hash != expected.wasm_hash {
        bail!(
            "WASM hash mismatch! Expected {}…, got {}…",
            &expected.wasm_hash[..12],
            &actual_hash[..12]
        );
    }

    println!("  {} Bundle integrity verified", "✓".green());
    Ok(())
}

fn chrono_now_or_fallback() -> String {
    // Simple RFC3339-like timestamp without requiring chrono
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "2026-01-01T00:00:00Z".to_string(),
    }
}
