//! Repository archive retention picker (`SPC o o`). The picker owns only UI state.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    pub choices: Vec<i64>,
    pub cursor: usize,
    pub now: i64,
    /// The value an unwritten `archive_days` stands at — the caller hands it in (`config::ARCHIVE_DAYS`), so the
    /// picker knows neither the config nor the store.
    pub default: i64,
    pub typed: String,
    pub error: Option<String>,
}

impl Picker {
    pub fn open(now: i64, default: i64) -> Self {
        let mut choices = vec![0, 7, 30, 60, 90, 180, 365];
        for one in [default, now] {
            if !choices.contains(&one) {
                choices.push(one);
            }
        }
        choices.sort_unstable();
        let cursor = choices.iter().position(|n| *n == now).unwrap_or(0);
        Self { choices, cursor, now, default, typed: String::new(), error: None }
    }

    pub fn selected(&self) -> Result<i64, &'static str> {
        if self.typed.is_empty() {
            Ok(self.choices[self.cursor])
        } else {
            self.typed.parse::<i64>().ok().filter(|n| (0..=3650).contains(n)).ok_or("0..3650")
        }
    }

    pub fn paste(&mut self, s: &str) {
        if s.chars().all(|c| c.is_ascii_digit()) {
            self.typed.push_str(s);
            self.error = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_custom_current_value_and_validates_input() {
        let mut p = Picker::open(21, 14);
        assert_eq!(p.choices, [0, 7, 14, 21, 30, 60, 90, 180, 365]);
        assert_eq!(p.selected(), Ok(21));
        p.paste("3650");
        assert_eq!(p.selected(), Ok(3650));
        p.paste("1");
        assert!(p.selected().is_err());
        p.typed.clear();
        p.paste("abc");
        assert_eq!(p.selected(), Ok(21));
    }
}
