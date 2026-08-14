use crate::synth_engine::{Sample, StereoSample};

enum DisplayUnit {
    Percents(Sample),
    Db(Sample),
    Semitones(Sample),
    Semicents(Sample),
    Hz(Sample),
    Khz(Sample),
    Ms(Sample),
    Secs(Sample),
}

impl DisplayUnit {
    fn format(&self) -> String {
        match self {
            Self::Db(value) => format!("{:+.1} dB", value),
            Self::Percents(value) => format!("{:.0}%", value),
            Self::Semitones(value) => {
                if *value == 0.0 {
                    "0 st".to_string()
                } else {
                    format!("{:.2} st", value)
                }
            }
            Self::Semicents(value) => format!("{:.0} cents", value),
            Self::Khz(value) => format!("{:.2} kHz", value),
            Self::Hz(value) => {
                let precision = Self::hz_precision(*value);
                format!("{0:.1$} Hz", value, precision)
            }
            Self::Ms(value) => {
                if value.abs() < 10.0 {
                    format!("{:.1} ms", value)
                } else {
                    format!("{:.0} ms", value)
                }
            }
            Self::Secs(value) => format!("{:.2} s", value),
        }
    }

    fn format_input(&self) -> String {
        let (number, unit) = match self {
            Self::Db(value) => (format!("{:.1}", value), None),
            Self::Percents(value) => (format!("{:.0}", value), None),
            Self::Semitones(value) => (format!("{:.2}", value), None),
            Self::Semicents(value) => (format!("{:.0}", value), Some("cents")),
            Self::Khz(value) => (format!("{:.2}", value), Some("kHz")),
            Self::Hz(value) => {
                let precision = Self::hz_precision(*value);
                (format!("{0:.1$}", value, precision), None)
            }
            Self::Ms(value) => {
                let precision = if value.abs() < 10.0 { 1 } else { 0 };
                (format!("{value:.precision$}"), Some("ms"))
            }
            Self::Secs(value) => (format!("{:.2}", value), None),
        };

        let number = Self::trim_trailing_zeros(number);

        match unit {
            Some(unit) => format!("{number} {unit}"),
            None => number,
        }
    }

    fn hz_precision(value: Sample) -> usize {
        if value.abs() < 1.0 {
            2
        } else if value.abs() < 10.0 {
            1
        } else {
            0
        }
    }

    fn trim_trailing_zeros(mut s: String) -> String {
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        s
    }
}

#[derive(Clone, Copy)]
pub enum Units {
    Normalized,
    Db,
    Octaves,
    Frequency,
    Time,
}

impl Units {
    fn display_unit(&self, value: Sample) -> DisplayUnit {
        match self {
            Self::Db => DisplayUnit::Db(value),
            Self::Normalized => DisplayUnit::Percents(value * 100.0),
            Self::Octaves => {
                let st = value * 12.0;

                if st == 0.0 {
                    DisplayUnit::Semitones(0.0)
                } else if st.abs() < 1.0 {
                    DisplayUnit::Semicents(value * 1_200.0)
                } else {
                    DisplayUnit::Semitones(st)
                }
            }
            Self::Frequency => {
                if value.abs() > 1_000.0 {
                    DisplayUnit::Khz(value / 1_000.0)
                } else {
                    DisplayUnit::Hz(value)
                }
            }
            Self::Time => {
                let ms = value * 1_000.0;

                if ms.abs() < 1_000.0 {
                    DisplayUnit::Ms(ms)
                } else {
                    DisplayUnit::Secs(value)
                }
            }
        }
    }

    pub fn format(&self, value: Sample) -> String {
        self.display_unit(value).format()
    }

    pub(crate) fn format_input(&self, value: Sample) -> String {
        self.display_unit(value).format_input()
    }

    pub(crate) fn parse(&self, text: &str, stereo: bool) -> Option<StereoSample> {
        let parts: Vec<&str> = text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();

        match parts.as_slice() {
            [one] => self.parse_one(one).map(StereoSample::splat),
            [left, right] if stereo => Some(StereoSample::new(
                self.parse_one(left)?,
                self.parse_one(right)?,
            )),
            _ => None,
        }
    }

    fn parse_one(&self, text: &str) -> Option<Sample> {
        let (number, unit) = split_number_and_unit(text)?;

        match self {
            Self::Normalized if unit.is_empty() || unit == "%" => Some(number / 100.0),
            Self::Db if unit.is_empty() || unit.eq_ignore_ascii_case("db") => Some(number),
            Self::Octaves if unit.is_empty() || unit.eq_ignore_ascii_case("st") => {
                Some(number / 12.0)
            }
            Self::Octaves
                if unit.eq_ignore_ascii_case("cents") || unit.eq_ignore_ascii_case("cent") =>
            {
                Some(number / 1_200.0)
            }
            Self::Frequency if unit.is_empty() || unit.eq_ignore_ascii_case("hz") => Some(number),
            Self::Frequency if unit.eq_ignore_ascii_case("khz") => Some(number * 1_000.0),
            Self::Time if unit.is_empty() || unit.eq_ignore_ascii_case("s") => Some(number),
            Self::Time if unit.eq_ignore_ascii_case("ms") => Some(number / 1_000.0),
            _ => None,
        }
    }
}

fn split_number_and_unit(text: &str) -> Option<(Sample, &str)> {
    let text = text.trim();
    let bytes = text.as_bytes();
    let mut i = 0;

    if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
        i += 1;
    }

    let number_start = i;

    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }

    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }

    if i == number_start {
        return None;
    }

    let number = text[..i].parse().ok()?;
    Some((number, text[i..].trim()))
}

#[cfg(test)]
mod tests;
