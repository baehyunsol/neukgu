pub fn prettify_tokens(t: u64) -> String {
    match t {
        ..10_000 => t.to_string(),
        ..10_000_000 => format!("{}K", t / 1000),
        ..10_000_000_000 => format!("{}M", t / 1_000_000),
        _ => format!("{}B", t / 1_000_000_000),
    }
}

pub fn prettify_bytes(b: u64) -> String {
    match b {
        0..=19_999 => format!("{b} bytes"),
        20_000..=19_999_999 => format!("{} KiB", b >> 10),
        20_000_000..=19_999_999_999 => format!("{} MiB", b >> 20),
        _ => format!("{} GiB", b >> 30),
    }
}

pub fn prettify_time(ms: u64) -> String {
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let weeks = days / 7;

    // A month is roughly 30.437 days.
    // This *rough* estimation makes sense only if `days` is large!
    let months = days * 1000 / 30437;
    let years = months / 12;

    if seconds < 10 {
        format!("{:.2} seconds", ms as f64 / 1000.0)
    } else if seconds < 120 {
        format!("{seconds} seconds")
    } else if hours < 2 {
        format!("{minutes} minutes {} seconds", seconds % 60)
    } else if hours < 20 {
        format!("{hours} hours {} minutes", minutes % 60)
    } else if days < 2 {
        format!("{hours} hours")
    } else if days < 20 {
        format!("{days} days {} hours", hours % 24)
    } else if days < 100 {
        format!("{days} days")
    } else if weeks < 40 {
        format!("{weeks} weeks")
    } else if months < 30 {
        format!("{months} months")
    } else {
        format!("{years} years {} months", months % 12)
    }
}
