// Network Manager - Auto-Switch Service
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Auto-switch service for automatic profile activation based on rules.
//!
//! This service periodically evaluates rule conditions and triggers
//! profile switches when conditions match.

use crate::models::rules::{Condition, InterfaceStateMatch, RuleOperator, RuleSet};
use crate::models::Profile;
use chrono::{Datelike, Local, Timelike};
use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, info, warn};

/// Maximum compiled regex size to prevent ReDoS (1 KiB).
const REGEX_SIZE_LIMIT: usize = 1 << 10;

/// Service for evaluating auto-switch rules and triggering profile changes.
pub struct AutoSwitchService {
    /// Current network SSID (cached).
    cached_ssid: Option<String>,
    /// Current gateway MAC (cached).
    cached_gateway_mac: Option<String>,
    /// Last evaluated profile ID.
    last_profile_id: Option<String>,
    /// Compiled regex cache keyed by pattern string.
    regex_cache: HashMap<String, regex::Regex>,
}

impl AutoSwitchService {
    /// Create a new auto-switch service.
    pub fn new() -> Self {
        Self {
            cached_ssid: None,
            cached_gateway_mac: None,
            last_profile_id: None,
            regex_cache: HashMap::new(),
        }
    }

    /// Get or compile a regex, using the cache and applying a size limit.
    fn get_or_compile_regex(&mut self, pattern: &str) -> Option<&regex::Regex> {
        // Insert if not present
        if !self.regex_cache.contains_key(pattern) {
            match regex::RegexBuilder::new(pattern)
                .size_limit(REGEX_SIZE_LIMIT)
                .build()
            {
                Ok(re) => {
                    self.regex_cache.insert(pattern.to_string(), re);
                }
                Err(e) => {
                    warn!("Failed to compile regex '{}': {}", pattern, e);
                    return None;
                }
            }
        }
        self.regex_cache.get(pattern)
    }

    /// Evaluate all profiles and return the ID of the first matching profile.
    pub fn evaluate_profiles(&mut self, profiles: &[Profile]) -> Option<String> {
        // Update cached network state
        self.update_network_state();

        // Sort profiles by priority (higher first)
        let mut sorted_profiles: Vec<_> = profiles
            .iter()
            .filter(|p| {
                p.auto_switch_rules
                    .as_ref()
                    .map(|r| r.enabled && !r.conditions.is_empty())
                    .unwrap_or(false)
            })
            .collect();
        
        sorted_profiles.sort_by(|a, b| {
            let a_priority = a.auto_switch_rules.as_ref().map(|r| r.priority).unwrap_or(0);
            let b_priority = b.auto_switch_rules.as_ref().map(|r| r.priority).unwrap_or(0);
            b_priority.cmp(&a_priority)
        });

        // Evaluate each profile's rules
        for profile in sorted_profiles {
            let Some(rules) = profile.auto_switch_rules.as_ref() else {
                continue;
            };
            
            if self.evaluate_ruleset(rules) {
                // Skip if already active
                if self.last_profile_id.as_ref() == Some(&profile.id().to_string()) {
                    debug!("Profile {} already active, skipping", profile.name());
                    return None;
                }

                info!("Auto-switch: Profile '{}' matches rules", profile.name());
                self.last_profile_id = Some(profile.id().to_string());
                return Some(profile.id().to_string());
            }
        }

        None
    }

    /// Evaluate a rule set.
    fn evaluate_ruleset(&mut self, rules: &RuleSet) -> bool {
        if rules.conditions.is_empty() {
            return false;
        }

        match rules.operator {
            RuleOperator::And => {
                for c in &rules.conditions {
                    if !self.evaluate_condition(c) {
                        return false;
                    }
                }
                true
            }
            RuleOperator::Or => {
                for c in &rules.conditions {
                    if self.evaluate_condition(c) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Evaluate a single condition.
    fn evaluate_condition(&mut self, condition: &Condition) -> bool {
        match condition {
            Condition::WifiSsid { ssid, regex } => {
                self.check_wifi_ssid(ssid, *regex)
            }
            Condition::GatewayMac { mac } => {
                self.check_gateway_mac(mac)
            }
            Condition::PingTarget { host, timeout_ms } => {
                self.check_ping(host, *timeout_ms)
            }
            Condition::InterfaceState { interface, state } => {
                self.check_interface_state(interface, state)
            }
            Condition::TimeWindow { window } => {
                let now = Local::now();
                let time = chrono::NaiveTime::from_hms_opt(
                    now.hour(), now.minute(), now.second()
                ).unwrap_or_default();
                let weekday = now.weekday();
                window.is_active(time, weekday)
            }
            Condition::NetworkAvailable => {
                self.check_network_available()
            }
            Condition::Not { condition } => {
                !self.evaluate_condition(condition)
            }
        }
    }

    /// Update cached network state.
    fn update_network_state(&mut self) {
        // Get current SSID
        self.cached_ssid = Self::get_current_ssid();
        
        // Get gateway MAC
        self.cached_gateway_mac = Self::get_gateway_mac();
    }

    /// Get current Wi-Fi SSID.
    fn get_current_ssid() -> Option<String> {
        // Try nmcli first
        let output = Command::new("nmcli")
            .args(["-t", "-f", "active,ssid", "dev", "wifi"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.starts_with("yes:") {
                return Some(line.trim_start_matches("yes:").to_string());
            }
        }
        
        None
    }

    /// Get gateway MAC address.
    fn get_gateway_mac() -> Option<String> {
        // Get default gateway IP
        let output = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let gateway_ip = stdout
            .lines()
            .next()?
            .split_whitespace()
            .nth(2)?;

        // Get MAC from ARP table
        let arp_output = Command::new("ip")
            .args(["neigh", "show", gateway_ip])
            .output()
            .ok()?;

        let arp_stdout = String::from_utf8_lossy(&arp_output.stdout);
        arp_stdout
            .lines()
            .next()?
            .split_whitespace()
            .nth(4)
            .map(|s| s.to_lowercase())
    }

    /// Check Wi-Fi SSID condition.
    fn check_wifi_ssid(&mut self, ssid: &str, regex: bool) -> bool {
        let Some(current) = &self.cached_ssid.clone() else {
            return false;
        };

        if regex {
            // Use regex matching with size-limited cache
            self.get_or_compile_regex(ssid)
                .map(|re| re.is_match(current))
                .unwrap_or(false)
        } else {
            // Glob-style matching (simple * support)
            if ssid.contains('*') {
                let pattern = format!("^{}$", ssid.replace('*', ".*"));
                self.get_or_compile_regex(&pattern)
                    .map(|re| re.is_match(current))
                    .unwrap_or(false)
            } else {
                current == ssid
            }
        }
    }

    /// Check gateway MAC condition.
    fn check_gateway_mac(&self, mac: &str) -> bool {
        self.cached_gateway_mac
            .as_ref()
            .map(|m| m.to_lowercase() == mac.to_lowercase())
            .unwrap_or(false)
    }

    /// Check ping target reachability.
    fn check_ping(&self, host: &str, timeout_ms: u32) -> bool {
        let timeout_secs = (timeout_ms / 1000).max(1);
        
        Command::new("ping")
            .args(["-c", "1", "-W", &timeout_secs.to_string(), host])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Check interface state.
    fn check_interface_state(&self, interface: &str, expected: &InterfaceStateMatch) -> bool {
        let operstate_path = format!("/sys/class/net/{}/operstate", interface);
        let carrier_path = format!("/sys/class/net/{}/carrier", interface);

        match expected {
            InterfaceStateMatch::Up => {
                std::fs::read_to_string(&operstate_path)
                    .map(|s| s.trim() == "up")
                    .unwrap_or(false)
            }
            InterfaceStateMatch::Down => {
                std::fs::read_to_string(&operstate_path)
                    .map(|s| s.trim() == "down")
                    .unwrap_or(false)
            }
            InterfaceStateMatch::Carrier => {
                std::fs::read_to_string(&carrier_path)
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false)
            }
            InterfaceStateMatch::NoCarrier => {
                std::fs::read_to_string(&carrier_path)
                    .map(|s| s.trim() == "0")
                    .unwrap_or(true)
            }
        }
    }

    /// Check if any network is available.
    fn check_network_available(&self) -> bool {
        // Check if connected to any network
        Command::new("nmcli")
            .args(["general", "status"])
            .output()
            .map(|out| {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.contains("connected")
            })
            .unwrap_or(false)
    }

    /// Clear the last profile ID (used when profile is manually changed).
    #[allow(dead_code)]
    pub fn clear_last_profile(&mut self) {
        self.last_profile_id = None;
        self.regex_cache.clear();
    }
}

impl Default for AutoSwitchService {
    fn default() -> Self {
        Self::new()
    }
}
