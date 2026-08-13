//! cash lib: трекер расходов поверх eml-tag.
//!
//! Запись: X-Tag: <категория>, X-Timestamp: ts, тело {"cents": i64, "note"}.
//! Суммы в целых копейках (без float).

use emlbox::format::find_blank;
use emlbox::tagdb;
use serde_json::Value;
use std::path::Path;

pub fn body_of(p: &Path) -> Result<Value, String> {
    let data = std::fs::read(p).map_err(|e| e.to_string())?;
    let (end, sep) = find_blank(&data).ok_or("no blank line in record")?;
    serde_json::from_slice(&data[end + sep..]).map_err(|e| e.to_string())
}

/// "1234.56" -> 123456 cents. Up to 2 decimals; extra digits rejected.
pub fn parse_amount(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty amount".into());
    }
    let neg = s.starts_with('-');
    let s = s.trim_start_matches(['-', '+']);
    let mut parts = s.splitn(2, '.');
    let rub: i64 = parts.next().unwrap_or("0").parse().map_err(|_| format!("bad amount: {s}"))?;
    let cop: i64 = match parts.next() {
        None => 0,
        Some(c) => {
            if c.len() > 2 {
                return Err(format!("too many decimals: {c}"));
            }
            let padded = format!("{c:0<2}");
            padded.parse().map_err(|_| format!("bad kopecks: {c}"))?
        }
    };
    let cents = rub * 100 + cop;
    Ok(if neg { -cents } else { cents })
}

pub fn fmt_cents(cents: i64) -> String {
    let neg = cents < 0;
    let a = cents.unsigned_abs();
    let rub = a / 100;
    let cop = a % 100;
    format!("{}{}.{:02}", if neg { "-" } else { "" }, rub, cop)
}

pub fn add(db: &Path, cents: i64, category: &str, note: &str, ts: u64) -> Result<String, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let id = format!("cx_{nanos}");
    let body = serde_json::json!({"cents": cents, "note": note});
    tagdb::insert(db, &id, &[category.to_string()], "local", ts, &body).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Civil date (proleptic Gregorian, Hinnant). Shared with track.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn day_bounds(date: &str) -> Result<(u64, u64), String> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("bad date: {date} (need YYYY-MM-DD)"));
    }
    let y: i64 = parts[0].parse().map_err(|_| "bad year")?;
    let m: i64 = parts[1].parse().map_err(|_| "bad month")?;
    let d: i64 = parts[2].parse().map_err(|_| "bad day")?;
    let start = days_from_civil(y, m, d) as u64 * 86400;
    Ok((start, start + 86400))
}

pub fn month_bounds(ym: &str) -> Result<(u64, u64), String> {
    let parts: Vec<&str> = ym.split('-').collect();
    if parts.len() != 2 {
        return Err(format!("bad month: {ym} (need YYYY-MM)"));
    }
    let y: i64 = parts[0].parse().map_err(|_| "bad year")?;
    let m: i64 = parts[1].parse().map_err(|_| "bad month")?;
    if !(1..=12).contains(&m) {
        return Err(format!("bad month: {m}"));
    }
    let start = days_from_civil(y, m, 1) as u64 * 86400;
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end = days_from_civil(ny, nm, 1) as u64 * 86400;
    Ok((start, end))
}

/// Sum per category in [from, to). Returns (category, cents, count).
pub fn summary(db: &Path, from: u64, to: u64) -> Result<Vec<(String, i64, usize)>, String> {
    let q = format!("X-Timestamp >= {from} AND X-Timestamp < {to}");
    let (records, _) = tagdb::query(db, &q)?;
    let mut acc: std::collections::HashMap<String, (i64, usize)> = std::collections::HashMap::new();
    for (p, headers) in records {
        let cat = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("X-Tag"))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "misc".to_string());
        if let Ok(b) = body_of(&p) {
            if let Some(c) = b.get("cents").and_then(|v| v.as_i64()) {
                let e = acc.entry(cat).or_insert((0, 0));
                e.0 += c;
                e.1 += 1;
            }
        }
    }
    let mut out: Vec<_> = acc.into_iter().collect();
    out.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    Ok(out
        .into_iter()
        .map(|(cat, (cents, n))| (cat, cents, n))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(tag: &str) -> PathBuf {
        let db = std::env::temp_dir().join(format!("cash_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&db);
        std::fs::create_dir_all(&db).unwrap();
        db
    }

    #[test]
    fn amounts() {
        assert_eq!(parse_amount("12.34").unwrap(), 1234);
        assert_eq!(parse_amount("12.3").unwrap(), 1230);
        assert_eq!(parse_amount("12").unwrap(), 1200);
        assert_eq!(parse_amount("0.01").unwrap(), 1);
        assert_eq!(parse_amount("-5.00").unwrap(), -500);
        assert!(parse_amount("1.234").is_err());
        assert!(parse_amount("abc").is_err());
        assert_eq!(fmt_cents(1234), "12.34");
        assert_eq!(fmt_cents(-500), "-5.00");
        assert_eq!(fmt_cents(0), "0.00");
    }

    #[test]
    fn summary_by_category() {
        let db = setup("s");
        add(&db, 5000, "food", "", 100).unwrap();
        add(&db, 1234, "food", "", 200).unwrap();
        add(&db, 9999, "rent", "", 300).unwrap();
        let s = summary(&db, 0, 1000).unwrap();
        assert_eq!(s, vec![
            ("rent".to_string(), 9999, 1),
            ("food".to_string(), 6234, 2),
        ]);
    }

    #[test]
    fn month_bounds_work() {
        let (a, b) = month_bounds("2026-08").unwrap();
        assert_eq!(a, days_from_civil(2026, 8, 1) as u64 * 86400);
        assert_eq!(b - a, 31 * 86400);
        let (a, b) = month_bounds("2026-02").unwrap();
        assert_eq!(b - a, 28 * 86400);
        let (a, b) = month_bounds("2026-12").unwrap();
        assert_eq!(b - a, 31 * 86400);
        let (a, b) = month_bounds("2024-02").unwrap(); // leap
        assert_eq!(b - a, 29 * 86400);
    }
}
