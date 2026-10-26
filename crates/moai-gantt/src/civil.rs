//! 날 번호 ↔ (년, 월, 일). Howard Hinnant 의 `civil_from_days`·`days_from_civil` 이다.
//!
//! moai 의 `model::civil_from_days` 와 같은 식을 일부러 한 번 더 둔다 — 이 크레이트는 moai 를
//! 모르고, chrono 를 들이는 것보다 열 줄이 싸다. 날 번호 0 은 1970-01-01 이다.

/// 날 번호를 (년, 월, 일) 로.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (년, 월, 일) 을 날 번호로.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_day_zero() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn leap_day_of_2000_exists() {
        // 400 으로 나뉘는 해는 윤년이다.
        let d = days_from_civil(2000, 2, 29);
        assert_eq!(civil_from_days(d), (2000, 2, 29));
        assert_eq!(civil_from_days(d + 1), (2000, 3, 1));
    }

    #[test]
    fn year_1900_is_not_leap() {
        // 100 으로 나뉘나 400 으로 안 나뉘는 해는 윤년이 아니다 — 2월 28일 다음이 3월 1일이다.
        let d = days_from_civil(1900, 2, 28);
        assert_eq!(civil_from_days(d + 1), (1900, 3, 1));
    }

    #[test]
    fn ordinary_leap_year() {
        let d = days_from_civil(2024, 2, 28);
        assert_eq!(civil_from_days(d + 1), (2024, 2, 29));
        assert_eq!(civil_from_days(d + 2), (2024, 3, 1));
    }

    #[test]
    fn negative_days_count_back_before_the_epoch() {
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(civil_from_days(-719_468), (0, 3, 1));
    }

    #[test]
    fn roundtrips_over_a_wide_range() {
        for z in (-800_000..800_000).step_by(37) {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn known_recent_day() {
        assert_eq!(days_from_civil(2026, 10, 25), 20_751);
    }
}
