//! Unix time and ISO 8601 dates, in UTC, with the proleptic Gregorian
//! calendar. The day arithmetic is Howard Hinnant's.

/// Days since 1970-01-01 for a civil date.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i64> {
    let y = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era.checked_mul(146_097)?
        .checked_add(doe)?
        .checked_sub(719_468)
}

/// The civil date for days since 1970-01-01.
pub fn civil_from_days(days: i64) -> Option<(i64, u32, u32)> {
    let z = days.checked_add(719_468)?;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).ok()?;
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).ok()?;
    let year = era.checked_mul(400)?.checked_add(yoe)?;
    Some((
        if month <= 2 {
            year.checked_add(1)?
        } else {
            year
        },
        month,
        day,
    ))
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

/// `2024-09-26T14:40:00Z`, with a fraction (`nanos`, given as a count and
/// how many digits to show) when it is not zero. None outside years 1 to
/// 9999.
pub fn format_utc(seconds: i64, fraction: Option<(u64, usize)>) -> Option<String> {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    if !(1..=9999).contains(&year) {
        return None;
    }
    let (h, m, s) = (rest / 3600, rest % 3600 / 60, rest % 60);
    let mut out = format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}");
    if let Some((value, digits)) = fraction
        && value > 0
    {
        out.push_str(&format!(".{value:0digits$}"));
    }
    out.push('Z');
    Some(out)
}

/// Reads an ISO 8601 date or date-time starting at byte `at`, which must
/// be on a char boundary. `YYYY-MM-DD`, then optionally `T` or a space and
/// `HH:MM`, `:SS`, `.fraction`, and `Z` or a `+HH:MM` offset. Without an
/// offset the time is UTC. Returns where it ended and the Unix seconds.
pub fn parse_iso(text: &str, at: usize) -> Option<(usize, i64)> {
    let b = text.as_bytes();
    let digits = |from: usize, count: usize| -> Option<i64> {
        let slice = b.get(from..from + count)?;
        if !slice.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(slice).ok()?.parse().ok()
    };
    let year = digits(at, 4)?;
    if b.get(at + 4) != Some(&b'-') {
        return None;
    }
    let month = u32::try_from(digits(at + 5, 2)?).ok()?;
    if b.get(at + 7) != Some(&b'-') {
        return None;
    }
    let day = u32::try_from(digits(at + 8, 2)?).ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let mut end = at + 10;
    let mut seconds_of_day: i64 = 0;
    let mut offset: i64 = 0;
    if matches!(b.get(end), Some(b'T' | b't' | b' '))
        && let (Some(h), Some(b':'), Some(m)) =
            (digits(end + 1, 2), b.get(end + 3), digits(end + 4, 2))
    {
        if h > 23 || m > 59 {
            return None;
        }
        end += 6;
        seconds_of_day = h * 3600 + m * 60;
        if b.get(end) == Some(&b':') {
            let s = digits(end + 1, 2)?;
            if s > 60 {
                return None;
            }
            seconds_of_day += s.min(59);
            end += 3;
            if b.get(end) == Some(&b'.') && b.get(end + 1).is_some_and(u8::is_ascii_digit) {
                end += 1;
                while b.get(end).is_some_and(u8::is_ascii_digit) {
                    end += 1;
                }
            }
        }
        match b.get(end) {
            Some(b'Z' | b'z') => end += 1,
            Some(sign @ (b'+' | b'-')) => {
                let oh = digits(end + 1, 2)?;
                let (om, len) = if b.get(end + 3) == Some(&b':') {
                    (digits(end + 4, 2)?, 6)
                } else if let Some(om) = digits(end + 3, 2) {
                    (om, 5)
                } else {
                    (0, 3)
                };
                if oh > 23 || om > 59 {
                    return None;
                }
                offset = oh * 3600 + om * 60;
                if *sign == b'-' {
                    offset = -offset;
                }
                end += len;
            }
            _ => {}
        }
    }
    if text[end..]
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric())
    {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(seconds_of_day)?
        .checked_sub(offset)?;
    Some((end, seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_leap_years() {
        assert_eq!(civil_from_days(0), Some((1970, 1, 1)));
        assert_eq!(days_from_civil(1970, 1, 1), Some(0));
        assert_eq!(
            format_utc(1_727_361_600, None).as_deref(),
            Some("2024-09-26T14:40:00Z")
        );
        assert_eq!(
            format_utc(951_782_400, None).as_deref(),
            Some("2000-02-29T00:00:00Z")
        );
        assert_eq!(
            format_utc(-1, None).as_deref(),
            Some("1969-12-31T23:59:59Z")
        );
        assert_eq!(
            format_utc(1_700_000_000, Some((123, 3))).as_deref(),
            Some("2023-11-14T22:13:20.123Z")
        );
        assert_eq!(
            format_utc(1_700_000_000, Some((0, 3))).as_deref(),
            Some("2023-11-14T22:13:20Z")
        );
        assert_eq!(format_utc(i64::MAX / 4, None), None);
    }

    #[test]
    fn parses_dates_and_offsets() {
        assert_eq!(parse_iso("2024-09-26", 0), Some((10, 1_727_308_800)));
        assert_eq!(
            parse_iso("2024-09-26T14:40:00Z", 0),
            Some((20, 1_727_361_600))
        );
        assert_eq!(parse_iso("2024-09-26 14:40", 0), Some((16, 1_727_361_600)));
        assert_eq!(
            parse_iso("2024-09-26T16:40:00.5+02:00", 0),
            Some((27, 1_727_361_600))
        );
        assert_eq!(
            parse_iso("2024-09-26T09:40:00-0500", 0),
            Some((24, 1_727_361_600))
        );
        assert_eq!(parse_iso("2024-09-26 hello", 0), Some((10, 1_727_308_800)));
        assert_eq!(parse_iso("2023-02-29", 0), None);
        assert_eq!(parse_iso("2024-13-01", 0), None);
        assert_eq!(parse_iso("2024-09-26x", 0), None);
    }
}
