//! lockin-helper: the small program that blocks websites, as root.
//!
//! Only root may change /etc/hosts. The daemon starts this helper through
//! polkit (`pkexec /usr/local/lib/lockin/lockin-helper block youtube.com`), and
//! a polkit rule installed by tools/install-helper.sh lets your user do that
//! without a password. It can do nothing else: change Lockin's own section of
//! /etc/hosts (see `hosts.rs`), with checked domain names.
//!
//!   lockin-helper block <domain>...   block exactly these websites
//!   lockin-helper unblock             remove Lockin's section
//!   lockin-helper show                print what's blocked now

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lockin_helper::hosts;

const USAGE: &str = "usage: lockin-helper block <domain>... | unblock | show";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("lockin-helper: {message}");
            ExitCode::FAILURE
        }
    }
}

/// The hosts file. LOCKIN_HOSTS_FILE points it elsewhere for tests; pkexec
/// clears the environment, so through polkit it's always /etc/hosts.
fn hosts_file() -> PathBuf {
    std::env::var_os("LOCKIN_HOSTS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/hosts"))
}

fn run(args: &[String]) -> Result<(), String> {
    let path = hosts_file();
    let current = std::fs::read_to_string(&path)
        .map_err(|e| format!("can't read {}: {e}", path.display()))?;
    match args.first().map(String::as_str) {
        Some("show") => {
            for domain in hosts::managed_domains(&current) {
                println!("{domain}");
            }
            Ok(())
        }
        Some("unblock") if args.len() == 1 => {
            write(&path, &current, &hosts::with_blocked(&current, &[]))
        }
        Some("block") if args.len() > 1 => {
            if args.len() - 1 > hosts::MAX_DOMAINS {
                return Err(format!("at most {} websites", hosts::MAX_DOMAINS));
            }
            let mut domains = Vec::new();
            for arg in &args[1..] {
                // Only clean domain names, exactly as the daemon sends them.
                match hosts::clean_domain(arg) {
                    Some(domain) if domain == *arg => {
                        if !domains.contains(&domain) {
                            domains.push(domain);
                        }
                    }
                    _ => return Err(format!("not a website: {arg}")),
                }
            }
            write(&path, &current, &hosts::with_blocked(&current, &domains))
        }
        _ => Err(USAGE.to_string()),
    }
}

/// Replace the file in one step: write a copy next to it, then rename it over
/// the original, so a crash never leaves half a hosts file.
fn write(path: &Path, current: &str, new: &str) -> Result<(), String> {
    if new == current {
        return Ok(());
    }
    // A link (rare) is followed, so the real file is changed.
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let temporary = target.with_extension("lockin-new");
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(new.as_bytes())?;
        file.sync_all()?;
        // The same permissions as the original (normally 644: everyone reads it).
        let permissions = std::fs::metadata(&target)?.permissions();
        std::fs::set_permissions(&temporary, permissions)?;
        std::fs::rename(&temporary, &target)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("can't write {}: {error}", target.display()));
    }
    Ok(())
}
