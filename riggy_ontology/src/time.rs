use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub struct GameTime {
    total_seconds: u32,
}

impl GameTime {
    pub const fn from_seconds(total_seconds: u32) -> Self {
        Self { total_seconds }
    }

    pub const fn seconds(self) -> u32 {
        self.total_seconds
    }

    pub fn advance(self, delta: TimeDelta) -> Self {
        Self {
            total_seconds: self.total_seconds.saturating_add(delta.seconds),
        }
    }

    pub fn format(self) -> String {
        let seconds_per_day = 24 * 60 * 60;
        let day = self.total_seconds / seconds_per_day + 1;
        let seconds_in_day = self.total_seconds % seconds_per_day;
        let hours = seconds_in_day / 3600;
        let minutes = (seconds_in_day % 3600) / 60;
        let seconds = seconds_in_day % 60;
        format!("Day {} {:02}:{:02}:{:02}", day, hours, minutes, seconds)
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub struct TimeDelta {
    seconds: u32,
}

impl TimeDelta {
    pub const ZERO: Self = Self { seconds: 0 };
    pub const ONE_SECOND: Self = Self { seconds: 1 };

    pub const fn from_seconds(seconds: u32) -> Self {
        Self { seconds }
    }

    pub const fn from_minutes(minutes: u32) -> Self {
        Self {
            seconds: minutes.saturating_mul(60),
        }
    }

    pub const fn from_hours(hours: u32) -> Self {
        Self {
            seconds: hours.saturating_mul(60).saturating_mul(60),
        }
    }

    pub const fn seconds(self) -> u32 {
        self.seconds
    }

    pub fn max(self, other: Self) -> Self {
        if self.seconds >= other.seconds {
            self
        } else {
            other
        }
    }

    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self {
            seconds: self.seconds.clamp(min.seconds, max.seconds),
        }
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            seconds: self.seconds.saturating_add(other.seconds),
        }
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            seconds: self.seconds.saturating_sub(other.seconds),
        }
    }

    pub fn format(self) -> String {
        let hours = self.seconds / 3600;
        let minutes = (self.seconds % 3600) / 60;
        let remainder = self.seconds % 60;
        if hours > 0 {
            format!("{hours}h {minutes:02}m {remainder:02}s")
        } else if minutes > 0 {
            format!("{minutes}m {remainder:02}s")
        } else {
            format!("{remainder}s")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GameTime, TimeDelta};

    #[test]
    fn game_time_formats_in_24_hour_clock() {
        assert_eq!(GameTime::from_seconds(29_430).format(), "Day 1 08:10:30");
    }

    #[test]
    fn time_delta_formats_compact_duration() {
        assert_eq!(TimeDelta::from_seconds(65).format(), "1m 05s");
        assert_eq!(TimeDelta::from_seconds(5).format(), "5s");
    }
}
