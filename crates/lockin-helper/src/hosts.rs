//! Blocking websites through `/etc/hosts`.
//!
//! Every program asks the system for a site's address, and the system looks in
//! /etc/hosts first. Pointing a site at 0.0.0.0 there blocks it in every browser
//! at once, without restarting them (after their DNS cache, about a minute).
//! Firefox respects /etc/hosts even with DNS-over-HTTPS on, and Chrome on Linux
//! asks the system.
//!
//! Lockin only ever touches its own section, between `BEGIN` and `END`; the rest
//! of the file stays as it is. The section names the domains it blocks on a
//! `# domains:` line, so the daemon can see what's blocked without guessing.
//!
//! The hosts file can't say "every subdomain", so each domain blocks itself plus
//! `www.` and `m.` (the mobile site). Other subdomains can be added as their own
//! website (music.youtube.com).

/// The first line of Lockin's section.
pub const BEGIN: &str =
    "# >>> Lockin: websites blocked during a study session (managed by Lockin, don't edit)";
/// The last line of Lockin's section.
pub const END: &str = "# <<< Lockin";
/// The line inside the section that lists the blocked domains.
const DOMAINS: &str = "# domains:";
/// At most this many websites at once, so a mistake can't grow the file without end.
pub const MAX_DOMAINS: usize = 500;

/// A website as typed ("https://www.YouTube.com/watch?v=1") turned into the
/// domain Lockin blocks ("youtube.com"). None when it isn't a domain name
/// (an IP address, "localhost", spaces, …).
pub fn clean_domain(input: &str) -> Option<String> {
    let mut text = input.trim().to_lowercase();
    // Drop the scheme, the path and the port.
    if let Some((_, rest)) = text.split_once("://") {
        text = rest.to_string();
    }
    if let Some((host, _)) = text.split_once(['/', '?', '#']) {
        text = host.to_string();
    }
    if let Some((host, _)) = text.split_once(':') {
        text = host.to_string();
    }
    let mut domain = text.trim_end_matches('.');
    // "*.reddit.com" and "www.reddit.com" both mean reddit.com.
    domain = domain.trim_start_matches("*.");
    domain = domain.strip_prefix("www.").unwrap_or(domain);

    let labels: Vec<&str> = domain.split('.').collect();
    let valid_label = |label: &&str| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    let top = labels.last()?;
    let ok = labels.len() >= 2
        && domain.len() <= 253
        && labels.iter().all(valid_label)
        // The last part is letters (".com", ".be"), so an IP address isn't a domain.
        && top.chars().all(|c| c.is_ascii_alphabetic());
    ok.then(|| domain.to_string())
}

/// The names one blocked domain stands for in the hosts file.
pub fn host_names(domain: &str) -> Vec<String> {
    vec![
        domain.to_string(),
        format!("www.{domain}"),
        format!("m.{domain}"),
    ]
}

/// The domains Lockin's section blocks now; empty without a section.
pub fn managed_domains(hosts: &str) -> Vec<String> {
    let mut inside = false;
    for line in hosts.lines() {
        let line = line.trim();
        if line == BEGIN {
            inside = true;
        } else if line == END {
            inside = false;
        } else if inside && let Some(list) = line.strip_prefix(DOMAINS) {
            return list.split_whitespace().map(str::to_string).collect();
        }
    }
    Vec::new()
}

/// The hosts file without Lockin's section.
pub fn without_section(hosts: &str) -> String {
    let mut kept = Vec::new();
    let mut inside = false;
    for line in hosts.lines() {
        match line.trim() {
            l if l == BEGIN => inside = true,
            l if l == END => inside = false,
            _ if inside => {}
            _ => kept.push(line),
        }
    }
    // No empty lines left behind at the end.
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut text = kept.join("\n");
    text.push('\n');
    text
}

/// The hosts file with exactly `domains` blocked in Lockin's section (none:
/// no section). The domains must already be clean (`clean_domain`).
pub fn with_blocked(hosts: &str, domains: &[String]) -> String {
    let mut text = without_section(hosts);
    if domains.is_empty() {
        return text;
    }
    text.push('\n');
    text.push_str(BEGIN);
    text.push('\n');
    text.push_str(&format!("{DOMAINS} {}\n", domains.join(" ")));
    for domain in domains {
        for name in host_names(domain) {
            // 0.0.0.0 and :: are "no address": the connection fails at once.
            text.push_str(&format!("0.0.0.0 {name}\n:: {name}\n"));
        }
    }
    text.push_str(END);
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_what_you_type() {
        assert_eq!(clean_domain("youtube.com").as_deref(), Some("youtube.com"));
        assert_eq!(
            clean_domain(" https://www.YouTube.com/watch?v=1 ").as_deref(),
            Some("youtube.com")
        );
        assert_eq!(clean_domain("*.reddit.com").as_deref(), Some("reddit.com"));
        assert_eq!(
            clean_domain("music.youtube.com:443").as_deref(),
            Some("music.youtube.com")
        );
        assert_eq!(clean_domain("x.com.").as_deref(), Some("x.com"));
    }

    #[test]
    fn refuses_what_isnt_a_website() {
        for input in [
            "",
            "localhost",
            "192.168.1.1",
            "you tube.com",
            "-bad.com",
            "a..com",
            "com",
        ] {
            assert_eq!(clean_domain(input), None, "{input}");
        }
    }

    const SYSTEM: &str = "127.0.0.1 localhost\n::1 localhost\n";

    #[test]
    fn adds_and_removes_only_its_own_section() {
        let domains = vec!["youtube.com".to_string(), "reddit.com".to_string()];
        let blocked = with_blocked(SYSTEM, &domains);
        assert!(blocked.starts_with(SYSTEM));
        assert!(blocked.contains("0.0.0.0 www.youtube.com\n"));
        assert!(blocked.contains(":: m.reddit.com\n"));
        assert_eq!(managed_domains(&blocked), domains);

        // Changing the list replaces the section, never adds a second one.
        let fewer = with_blocked(&blocked, &domains[..1]);
        assert_eq!(fewer.matches(BEGIN).count(), 1);
        assert_eq!(managed_domains(&fewer), vec!["youtube.com"]);

        // Nothing to block: the file is as it was.
        assert_eq!(with_blocked(&fewer, &[]), SYSTEM);
        assert!(managed_domains(SYSTEM).is_empty());
    }

    #[test]
    fn keeps_lines_after_the_section() {
        // Someone added a line below Lockin's section: it stays.
        let text =
            format!("{SYSTEM}\n{BEGIN}\n# domains: x.com\n0.0.0.0 x.com\n{END}\n10.0.0.2 nas\n");
        assert_eq!(without_section(&text), format!("{SYSTEM}\n10.0.0.2 nas\n"));
    }
}
