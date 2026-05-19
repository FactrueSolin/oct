mod backup;
mod bundle;
mod client;
mod config;
mod error;
mod paths;
mod token;

use clap::{Parser, Subcommand};
use error::Result;

use crate::bundle::ConfigBundle;
use crate::config::OctConfig;
use crate::paths::PathDiscovery;

const DEFAULT_ENDPOINT: &str = "https://oct.sereniblue.com";

#[derive(Parser)]
#[command(name = "oct", about = "OpenCode configuration sync CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize oct with a token
    Init {
        /// Provide an existing token
        #[arg(long)]
        token: Option<String>,

        /// Set the worker endpoint URL
        #[arg(long, default_value = DEFAULT_ENDPOINT)]
        endpoint: String,
    },
    /// Diagnose opencode config paths
    Doctor,
    /// Push local config to remote
    Push,
    /// Pull remote config to local
    Pull,
    /// Manage backups
    Back {
        /// List all backups
        #[arg(long)]
        list: bool,

        /// Restore a specific backup by ID
        #[arg(long)]
        id: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init { token, endpoint } => cmd_init(token, endpoint),
        Commands::Doctor => cmd_doctor(),
        Commands::Push => cmd_push(),
        Commands::Pull => cmd_pull(),
        Commands::Back { list, id } => cmd_back(list, id),
    }
}

fn cmd_init(provided_token: Option<String>, endpoint: String) -> Result<()> {
    let tok = match provided_token {
        Some(t) => {
            token::validate_token(&t)?;
            t
        }
        None => {
            let t = token::generate_token();
            token::validate_token(&t)?;
            t
        }
    };

    let cfg = OctConfig::new(tok.clone(), endpoint);
    cfg.save()?;

    println!("oct initialized successfully");
    println!("token: {}", tok);
    println!("endpoint: {}", cfg.endpoint);
    println!("config saved to: {}", config::oct_data_dir()?.join("config.toml").display());
    println!();
    println!("Copy this token to other machines to sync the same configuration.");

    Ok(())
}

fn cmd_doctor() -> Result<()> {
    let discovery = PathDiscovery::run()?;

    println!("=== Oct Path Discovery ===");
    println!();

    for p in &discovery.paths {
        let status = if p.exists { "found" } else { "not found" };
        println!("[{}] {} ({:?})", status, p.path.display(), p.kind);
    }

    println!();
    match &discovery.active_config {
        Some(p) => println!("active config dir: {}", p.display()),
        None => println!("active config dir: (none found)"),
    }
    match &discovery.active_data {
        Some(p) => println!("active data dir: {}", p.display()),
        None => println!("active data dir: (none found)"),
    }

    println!();

    match discovery.config_files() {
        Ok(files) => {
            println!("discovered {} config files:", files.len());
            for f in &files {
                println!("  {}", f.display());
            }
        }
        Err(e) => {
            println!("no config files found: {}", e);
        }
    }

    // Check oct local config
    println!();
    match OctConfig::load() {
        Ok(cfg) => {
            let masked = format!("{}****{}", &cfg.token[..4], &cfg.token[cfg.token.len() - 4..]);
            println!("oct config: loaded (token: {}, endpoint: {})", masked, cfg.endpoint);
        }
        Err(_) => {
            println!("oct config: not initialized (run `oct init`)");
        }
    }

    Ok(())
}

fn cmd_push() -> Result<()> {
    let cfg = OctConfig::load()?;

    println!("discovering config files...");
    let discovery = PathDiscovery::run()?;
    let bundle = ConfigBundle::from_discovery(&discovery)?;

    println!(
        "bundled {} files ({} bytes), hash: {}",
        bundle.files.len(),
        bundle.total_bytes,
        &bundle.bundle_hash[..16]
    );

    let meta = client::push_config(&cfg, &bundle)?;

    println!("push successful");
    println!("  revision: {}", meta.bundle_hash);
    println!("  files: {}", meta.file_count);
    println!("  size: {} bytes", meta.total_bytes);

    Ok(())
}

fn cmd_pull() -> Result<()> {
    let cfg = OctConfig::load()?;

    println!("fetching remote config...");
    let bundle = client::pull_config(&cfg)?;

    println!(
        "remote bundle: {} files ({} bytes), hash: {}",
        bundle.files.len(),
        bundle.total_bytes,
        &bundle.bundle_hash[..16]
    );

    // Determine target directories
    let discovery = PathDiscovery::run()?;
    let target_dirs: Vec<std::path::PathBuf> = discovery
        .active_config
        .iter()
        .chain(discovery.active_data.iter())
        .cloned()
        .collect();

    if target_dirs.is_empty() {
        return Err(error::OctError::NoConfigFound);
    }

    // Create backup before overwriting
    let mut files_to_backup = Vec::new();
    for entry in &bundle.files {
        for base in &target_dirs {
            let full = base.join(&entry.path);
            if full.exists() {
                files_to_backup.push((full, base.clone()));
            }
        }
    }

    if !files_to_backup.is_empty() {
        println!("creating backup of {} existing files...", files_to_backup.len());
        let manifest = backup::create_backup(&files_to_backup, "before-pull")?;
        println!("backup created: {}", manifest.id);
    }

    // Apply bundle to each target directory
    for base in &target_dirs {
        if base.exists() {
            println!("applying to {}...", base.display());
            bundle::apply_bundle(&bundle, base)?;
        }
    }

    println!("pull successful");
    Ok(())
}

fn cmd_back(list: bool, id: Option<String>) -> Result<()> {
    if list {
        let backups = backup::list_backups()?;
        if backups.is_empty() {
            println!("no backups found");
            return Ok(());
        }
        println!("=== Backups ===");
        for b in &backups {
            println!(
                "id: {} | created: {} | trigger: {} | files: {}",
                b.id, b.created_at, b.trigger, b.files.len()
            );
        }
        return Ok(());
    }

    if let Some(backup_id) = id {
        backup::restore_backup(&backup_id)?;
        return Ok(());
    }

    // Default: restore most recent backup
    let backups = backup::list_backups()?;
    if backups.is_empty() {
        println!("no backups found");
        return Ok(());
    }
    let latest = &backups[0];
    println!("restoring most recent backup: {}", latest.id);
    backup::restore_backup(&latest.id)?;

    Ok(())
}
