// Network Manager - Profile Scheduler
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Profile scheduling service.
//!
//! This module provides a scheduler that checks scheduled profile activations
//! and triggers them at the appropriate times.

use crate::models::ScheduleEntry;
use chrono::{Local, Datelike, Timelike};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Scheduler service for timed profile activations.
#[allow(dead_code)]
pub struct SchedulerService {
    running: Arc<AtomicBool>,
}

impl Default for SchedulerService {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SchedulerService {
    /// Create a new scheduler service.
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if a schedule should trigger now.
    /// 
    /// Cron format: minute hour day-of-month month day-of-week
    /// Supports: * for any, specific numbers, ranges (1-5), lists (1,3,5)
    pub fn should_trigger(schedule: &ScheduleEntry) -> bool {
        if !schedule.enabled {
            return false;
        }

        let now = Local::now();
        let parts: Vec<&str> = schedule.cron_expression.split_whitespace().collect();
        
        if parts.len() != 5 {
            return false;
        }

        let minute_match = Self::matches_field(parts[0], now.minute());
        let hour_match = Self::matches_field(parts[1], now.hour());
        let dom_match = Self::matches_field(parts[2], now.day());
        let month_match = Self::matches_field(parts[3], now.month());
        let dow_match = Self::matches_field(parts[4], now.weekday().num_days_from_sunday());

        minute_match && hour_match && dom_match && month_match && dow_match
    }

    /// Check if a cron field matches a value.
    fn matches_field(field: &str, value: u32) -> bool {
        if field == "*" {
            return true;
        }

        // Handle lists (e.g., "1,3,5")
        if field.contains(',') {
            return field.split(',')
                .any(|part| Self::matches_field(part.trim(), value));
        }

        // Handle ranges (e.g., "1-5")
        if field.contains('-') {
            let range: Vec<&str> = field.split('-').collect();
            if range.len() == 2 {
                if let (Ok(start), Ok(end)) = (range[0].parse::<u32>(), range[1].parse::<u32>()) {
                    return value >= start && value <= end;
                }
            }
            return false;
        }

        // Handle step values (e.g., "*/5")
        if field.starts_with("*/") {
            if let Ok(step) = field[2..].parse::<u32>() {
                return step > 0 && value % step == 0;
            }
            return false;
        }

        // Simple number
        if let Ok(num) = field.parse::<u32>() {
            return value == num;
        }

        false
    }

    /// Check all schedules and return profile IDs that should be activated.
    pub fn check_schedules(schedules: &[ScheduleEntry]) -> Vec<String> {
        schedules.iter()
            .filter(|s| Self::should_trigger(s))
            .map(|s| s.profile_id.clone())
            .collect()
    }

    /// Stop the scheduler.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the scheduler is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Parse a simple time string (HH:MM) into hour and minute.
#[allow(dead_code)]
pub fn parse_time(time_str: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 2 {
        if let (Ok(hour), Ok(minute)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            if hour < 24 && minute < 60 {
                return Some((hour, minute));
            }
        }
    }
    None
}

/// Create a cron expression for a specific time every day.
#[allow(dead_code)]
pub fn cron_daily_at(hour: u32, minute: u32) -> String {
    format!("{} {} * * *", minute, hour)
}

/// Create a cron expression for specific weekdays at a time.
/// days: comma-separated day numbers (0=Sunday, 1=Monday, etc.)
#[allow(dead_code)]
pub fn cron_weekdays_at(hour: u32, minute: u32, days: &str) -> String {
    format!("{} {} * * {}", minute, hour, days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_wildcard() {
        assert!(SchedulerService::matches_field("*", 5));
        assert!(SchedulerService::matches_field("*", 59));
    }

    #[test]
    fn test_matches_number() {
        assert!(SchedulerService::matches_field("5", 5));
        assert!(!SchedulerService::matches_field("5", 6));
    }

    #[test]
    fn test_matches_range() {
        assert!(SchedulerService::matches_field("1-5", 3));
        assert!(!SchedulerService::matches_field("1-5", 6));
    }

    #[test]
    fn test_matches_list() {
        assert!(SchedulerService::matches_field("1,3,5", 3));
        assert!(!SchedulerService::matches_field("1,3,5", 2));
    }

    #[test]
    fn test_matches_step() {
        assert!(SchedulerService::matches_field("*/5", 0));
        assert!(SchedulerService::matches_field("*/5", 15));
        assert!(!SchedulerService::matches_field("*/5", 7));
    }
}
