use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use lockdiff::{diff_lockfiles, Change, ChangeKind, Format};

#[derive(Clone, Copy, ValueEnum)]
enum FormatArg {
    Cargo,
    Npm,
}

#[derive(Parser)]
#[command(
    name = "lockdiff",
    about = "Diffs two snapshots of the same lockfile (Cargo.lock or package-lock.json)"
)]
struct Cli {
    /// The "before" lockfile snapshot.
    old: PathBuf,
    /// The "after" lockfile snapshot.
    new: PathBuf,
    /// Force the lockfile format instead of guessing from OLD's file name.
    #[arg(long, value_enum)]
    format: Option<FormatArg>,
    /// Emit changes as a JSON array instead of human-readable lines.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let old_content = match fs::read_to_string(&cli.old) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lockdiff: error: reading {}: {e}", cli.old.display());
            return ExitCode::from(2);
        }
    };
    let new_content = match fs::read_to_string(&cli.new) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lockdiff: error: reading {}: {e}", cli.new.display());
            return ExitCode::from(2);
        }
    };

    let format = match cli.format {
        Some(FormatArg::Cargo) => Format::Cargo,
        Some(FormatArg::Npm) => Format::Npm,
        None => {
            let name = cli.old.file_name().and_then(|n| n.to_str()).unwrap_or("");
            Format::detect(name)
        }
    };

    let changes = match diff_lockfiles(&old_content, &new_content, format) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lockdiff: error: {e}");
            return ExitCode::from(2);
        }
    };

    report(&changes, cli.json);

    if changes.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn report(changes: &[Change], json: bool) {
    if json {
        let out = serde_json::to_string_pretty(changes).unwrap_or_else(|_| "[]".to_string());
        println!("{out}");
        return;
    }

    if changes.is_empty() {
        println!("lockdiff: no changes");
        return;
    }

    let section = |kind: ChangeKind, title: &str| {
        let matching: Vec<&Change> = changes.iter().filter(|c| c.kind == kind).collect();
        if matching.is_empty() {
            return;
        }
        println!("{title} ({}):", matching.len());
        for c in matching {
            match (&c.old_version, &c.new_version) {
                (Some(old), Some(new)) => println!("  {} {old} -> {new}", c.name),
                (None, Some(new)) => println!("  {} {new}", c.name),
                (Some(old), None) => println!("  {} {old}", c.name),
                (None, None) => println!("  {}", c.name),
            }
        }
    };

    section(ChangeKind::Upgraded, "Upgraded");
    section(ChangeKind::Downgraded, "Downgraded");
    section(ChangeKind::Added, "Added");
    section(ChangeKind::Removed, "Removed");

    eprintln!("\nlockdiff: {} package(s) changed", changes.len());
}
