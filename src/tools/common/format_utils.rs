/// Format bytes to a human-readable string with customizable precision and format
pub fn format_bytes(bytes: u64, precision: usize, show_original: bool) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes >= GB as u64 {
        let formatted = format!("{:.precision$} GB", bytes as f64 / GB);
        if show_original {
            format!("{} ({bytes} bytes)", formatted)
        } else {
            formatted
        }
    } else if bytes >= MB as u64 {
        let formatted = format!("{:.precision$} MB", bytes as f64 / MB);
        if show_original {
            format!("{} ({bytes} bytes)", formatted)
        } else {
            formatted
        }
    } else if bytes >= KB as u64 {
        let formatted = format!("{:.precision$} KB", bytes as f64 / KB);
        if show_original {
            format!("{} ({bytes} bytes)", formatted)
        } else {
            formatted
        }
    } else if show_original {
        format!("{bytes} bytes")
    } else {
        format!("{} B", bytes)
    }
}
