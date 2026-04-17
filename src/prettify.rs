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

    if seconds < 60 {
        format!("{:.2} seconds", ms as f64 / 1000.0)
    } else if hours < 2 {
        format!("{minutes} minutes {} seconds", seconds % 60)
    } else {
        format!("{hours} hours {} minutes", minutes % 60)
    }
}
