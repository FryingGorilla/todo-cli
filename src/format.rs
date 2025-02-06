use std::cmp::max;

use colored::*;
use lazy_static::lazy_static;
use regex::Regex;

pub fn progress_bar(progress: f32) -> String {
    let total_len = 25;
    let progress_len = (progress.min(1.0) * (total_len as f32)).round() as u32;
    let percentage = format!(" {:.0}%", progress * 100.0);
    format!(
        "{}{}{}",
        "━".repeat(progress_len as usize).green(),
        "━"
            .repeat(total_len - progress_len as usize)
            .truecolor(64, 64, 64),
        percentage.cyan()
    )
}

pub fn format_duration(duration: i64) -> String {
    let sign = if duration >= 0 { "" } else { "-" };
    let mut remaining = duration.abs();
    let months = remaining / 86400 / 30;
    remaining -= months * 86400 * 30;
    let days = remaining / 86400;
    remaining -= days * 86400;
    let hours = remaining / 3600;
    remaining -= hours * 3600;
    let minutes = remaining / 60;
    remaining -= minutes * 60;

    let month_s = if months != 0 {
        format!("{}mo ", months).truecolor(120, 103, 205)
    } else {
        "".into()
    };
    let days_s = if days != 0 {
        format!("{}d ", days).cyan()
    } else {
        "".into()
    };

    let hour_s = if hours != 0 {
        format!("{}h ", hours).purple()
    } else {
        "".into()
    };
    let minutes_s = if minutes != 0 {
        format!("{}m ", minutes).yellow()
    } else {
        "".into()
    };
    let seconds_s = if remaining != 0
        || (month_s.len() == 0 && days_s.len() == 0 && hour_s.len() == 0 && minutes_s.len() == 0)
    {
        format!("{}s", remaining).green()
    } else {
        "".into()
    };

    format!(
        "{sign}{}{}{}{}{}",
        month_s, days_s, hour_s, minutes_s, seconds_s
    )
}

lazy_static! {
    static ref ansi_re: Regex = Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap();
}

pub fn strip_colors(s: &str) -> String {
    ansi_re.replace_all(s, "").to_string()
}

pub fn card(strings: Vec<(String, String)>) -> String {
    let max_len = strings
        .iter()
        .map(|(f, s)| (f.chars().count(), strip_colors(s).chars().count()))
        .reduce(|cur, s| (max(cur.0, s.0), max(cur.1, s.1)))
        .unwrap_or((0, 0));

    let content = &strings
        .iter()
        .map(|(f, s)| {
            format!(
                "┣━╾╌{:<width$}·{}{}{}┃\n",
                f,
                " ".repeat(2),
                s,
                " ".repeat(max_len.1 - strip_colors(s).chars().count()),
                width = max_len.0 + 2,
            )
        })
        .collect::<String>();

    let line_width = ansi_re
        .replace_all(content.lines().next().unwrap_or(""), "")
        .chars()
        .count();

    format!("┏{}┓\n", "━".repeat(line_width - 2))
        + content
        + &format!("┗{}┛", "━".repeat(line_width - 2))
}
