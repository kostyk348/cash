//! cash CLI — трекер расходов.
//!
//!   cash init <db>
//!   cash add <db> <amount> [--tag CAT] [--note S] [--ts N]
//!   cash day <db> [YYYY-MM-DD]
//!   cash month <db> [YYYY-MM]
//!   cash tag <db> <cat>
//!   cash report <db>

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(|s| s.as_str()) {
        Some("init") => match args.get(2) {
            Some(db) => match std::fs::create_dir_all(db) {
                Ok(()) => {
                    println!("init {}", db);
                    0
                }
                Err(e) => err(&e.to_string()),
            },
            None => err("init: need db dir"),
        },
        Some("add") => cmd_add(&args[2..]),
        Some("day") => cmd_day(&args[2..]),
        Some("month") => cmd_month(&args[2..]),
        Some("tag") => cmd_tag(&args[2..]),
        Some("report") => cmd_report(&args[2..]),
        _ => {
            eprintln!("cash: см. usage в шапке исходника");
            2
        }
    };
    std::process::exit(code);
}

fn flag<'a>(a: &'a [String], name: &str) -> Option<&'a String> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1))
}

fn cmd_add(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("add: need db"),
    };
    let amount = match a.get(1) {
        Some(x) => x.clone(),
        None => return err("add: need amount"),
    };
    let cents = match cash::parse_amount(&amount) {
        Ok(c) => c,
        Err(e) => return err(&e),
    };
    let category = flag(a, "--tag").cloned().unwrap_or_else(|| "misc".to_string());
    let note = flag(a, "--note").cloned().unwrap_or_default();
    let ts: u64 = flag(a, "--ts").and_then(|s| s.parse().ok()).unwrap_or_else(now);
    match cash::add(&db, cents, &category, &note, ts) {
        Ok(id) => {
            println!("added {} {} [{}] ({})", id, cash::fmt_cents(cents), category, note);
            0
        }
        Err(e) => err(&e),
    }
}

fn cmd_day(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("day: need db"),
    };
    let (from, to, label) = match a.get(1) {
        Some(d) => match cash::day_bounds(d) {
            Ok(b) => (b.0, b.1, d.clone()),
            Err(e) => return err(&e),
        },
        None => today_bounds(),
    };
    print_summary(&db, from, to, &label)
}

fn cmd_month(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("month: need db"),
    };
    let (from, to, label) = match a.get(1) {
        Some(m) => match cash::month_bounds(m) {
            Ok(b) => (b.0, b.1, m.clone()),
            Err(e) => return err(&e),
        },
        None => {
            let (y, m) = current_ym();
            let label = format!("{y:04}-{m:02}");
            let b = cash::month_bounds(&label).unwrap();
            (b.0, b.1, label)
        }
    };
    print_summary(&db, from, to, &label)
}

fn cmd_tag(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("tag: need db"),
    };
    let cat = match a.get(1) {
        Some(c) => c.clone(),
        None => return err("tag: need category"),
    };
    match cash::summary(&db, 0, u64::MAX) {
        Ok(s) => match s.iter().find(|(c, _, _)| c == &cat) {
            Some((_, cents, n)) => {
                println!("[{cat}] {}  ({} tx)", cash::fmt_cents(*cents), n);
                0
            }
            None => {
                println!("[{cat}] no data");
                0
            }
        },
        Err(e) => err(&e),
    }
}

fn cmd_report(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("report: need db"),
    };
    match cash::summary(&db, 0, u64::MAX) {
        Ok(s) => {
            let total: i64 = s.iter().map(|(_, c, _)| c).sum();
            println!("=== report (all time) ===");
            for (cat, cents, n) in &s {
                println!("  {:>10}  {}  ({} tx)", cash::fmt_cents(*cents), cat, n);
            }
            println!("  total: {}", cash::fmt_cents(total));
            0
        }
        Err(e) => err(&e),
    }
}

fn print_summary(db: &Path, from: u64, to: u64, label: &str) -> i32 {
    match cash::summary(db, from, to) {
        Ok(s) => {
            let total: i64 = s.iter().map(|(_, c, _)| c).sum();
            println!("=== {label} ===");
            for (cat, cents, n) in &s {
                println!("  {:>10}  {}  ({} tx)", cash::fmt_cents(*cents), cat, n);
            }
            println!("  total: {}", cash::fmt_cents(total));
            0
        }
        Err(e) => err(&e),
    }
}

fn today_bounds() -> (u64, u64, String) {
    let days = (now() as i64) / 86400;
    let (y, m, d) = civil_from_days(days);
    let label = format!("{y:04}-{m:02}-{d:02}");
    let b = cash::day_bounds(&label).unwrap();
    (b.0, b.1, label)
}

fn current_ym() -> (i64, i64) {
    let days = (now() as i64) / 86400;
    let (y, m, _) = civil_from_days(days);
    (y, m)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn err(e: &str) -> i32 {
    eprintln!("cash: {e}");
    1
}
