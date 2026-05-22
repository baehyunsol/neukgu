use chrono::Local;

pub fn prettify_tokens(t: u64) -> String {
    match t {
        ..1_000 => t.to_string(),
        ..10_000 => format!("{}.{}K", t / 1000, t / 100 % 10),
        ..1_000_000 => format!("{}K", t / 1000),
        ..10_000_000 => format!("{}.{}M", t / 1_000_000, t / 100_000 % 10),
        ..1_000_000_000 => format!("{}M", t / 1_000_000),
        ..10_000_000_000 => format!("{}.{}B", t / 1_000_000_000, t / 100_000_000 % 10),
        _ => format!("{}B", t / 1_000_000_000),
    }
}

pub fn prettify_bytes(b: u64) -> String {
    match b {
        ..1_024 => format!("{b} bytes"),
        ..10_240 => format!("{}.{} KiB", b >> 10, ((b * 10) >> 10) % 10),
        ..1_048_576 => format!("{} KiB", b >> 10),
        ..10_485_760 => format!("{}.{} MiB", b >> 20, ((b * 10) >> 20) % 10),
        ..1_073_741_824 => format!("{} MiB", b >> 20),
        ..10_737_418_240 => format!("{}.{} GiB", b >> 30, ((b * 10) >> 30) % 10),
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

    if seconds < 5 {
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

pub fn prettify_timestamp(timestamp_millis: i64) -> String {
    let now = Local::now().timestamp_millis();

    match now - timestamp_millis {
        ..0 => String::from("past"),
        ..5_000 => String::from("now"),
        d => format!("{} ago", prettify_time(d as u64)),
    }
}
