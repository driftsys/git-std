use crate::ui;

/// Compute today's [`standard_version::calver::CalverDate`] using the Howard
/// Hinnant civil_from_days algorithm (no external date crate needed).
pub(crate) fn today_calver_date() -> standard_version::calver::CalverDate {
    let secs = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => {
            ui::warning(&format!(
                "system clock failure ({e}), falling back to Unix epoch"
            ));
            0
        }
    };
    calver_date_from_epoch_days(secs.div_euclid(86400) as i32)
}

/// Compute a [`CalverDate`] from days since the Unix epoch.
pub(crate) fn calver_date_from_epoch_days(days: i32) -> standard_version::calver::CalverDate {
    // Howard Hinnant's civil_from_days algorithm.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let dow = ((days + 3).rem_euclid(7) + 1) as u32;
    let jan1_days = {
        let ys = y - 1;
        let eras = ys.div_euclid(400);
        let yoes = ys.rem_euclid(400) as u32;
        let ms: u32 = 10;
        let ds: u32 = 1;
        let doys = (153 * ms + 2) / 5 + ds - 1;
        let does = yoes * 365 + yoes / 4 - yoes / 100 + doys;
        eras * 146097 + does as i32 - 719468
    };
    let ordinal = days - jan1_days + 1;
    let jan1_dow = (jan1_days + 3).rem_euclid(7) + 1;

    let iso_week = {
        let w = (ordinal - dow as i32 + 10) / 7;
        if w < 1 {
            let prev_jan1_dow = (jan1_days - 1 + 3).rem_euclid(7) + 1;
            if prev_jan1_dow == 4
                || (prev_jan1_dow == 3 && {
                    let py = y - 1;
                    py % 4 == 0 && (py % 100 != 0 || py % 400 == 0)
                })
            {
                53
            } else {
                52
            }
        } else if w > 52 {
            let is_leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
            let days_in_year = if is_leap { 366 } else { 365 };
            if ordinal > days_in_year - 3 && jan1_dow != 4 {
                1
            } else {
                w
            }
        } else {
            w
        }
    };

    standard_version::calver::CalverDate {
        year: y as u32,
        month: m,
        day: d,
        iso_week: iso_week as u32,
        day_of_week: dow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_calver_date_is_reasonable() {
        let d = today_calver_date();
        assert!(d.year >= 2024);
        assert!((1..=12).contains(&d.month));
        assert!((1..=31).contains(&d.day));
        assert!((1..=53).contains(&d.iso_week));
        assert!((1..=7).contains(&d.day_of_week));
    }

    #[test]
    fn calver_date_2026_03_16() {
        let days = 20528;
        let d = calver_date_from_epoch_days(days);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 3);
        assert_eq!(d.day, 16);
        assert_eq!(d.day_of_week, 1);
        assert_eq!(d.iso_week, 12);
    }

    #[test]
    fn calver_date_dec31_to_jan1_boundary() {
        let dec31 = 20818;
        let d = calver_date_from_epoch_days(dec31);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 12);
        assert_eq!(d.day, 31);
        assert_eq!(d.day_of_week, 4);

        let jan1 = 20819;
        let d = calver_date_from_epoch_days(jan1);
        assert_eq!(d.year, 2027);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.day_of_week, 5);
    }

    #[test]
    fn calver_date_jan1_2024_monday() {
        let days = 19723;
        let d = calver_date_from_epoch_days(days);
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.day_of_week, 1);
        assert_eq!(d.iso_week, 1);
    }

    #[test]
    fn chrono_date_format() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let date = standard_changelog::format_date(secs);
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }
}
