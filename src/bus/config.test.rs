//! Tests for bus configuration types.
//!
//! The get_domains() method resolves domain subscriptions from two sources:
//! - `domains`: explicit Vec<String> (preferred)
//! - `domain`: single string supporting comma-separated values
//!
//! Why this matters: This dual-source pattern supports both YAML config
//! (domains array) and env vars (single comma-separated string). The fallback
//! logic enables simpler env var configuration while preserving full YAML
//! flexibility.
//!
//! Key behaviors verified:
//! - domains vec takes precedence over domain string
//! - domain string supports comma-separated values
//! - Whitespace is trimmed from comma-separated values
//! - Empty configurations return empty vec (publisher-only mode)

use super::*;

// ============================================================================
// IpcBusConfig::get_domains Tests
// ============================================================================

#[cfg(unix)]
mod ipc_config {
    use super::*;

    /// When domains vec is set, use it directly (preferred source).
    ///
    /// YAML config typically sets this directly as an array.
    #[test]
    fn test_get_domains_prefers_domains_over_domain() {
        let config = IpcBusConfig {
            domains: Some(vec!["player".to_string(), "table".to_string()]),
            domain: Some("hand".to_string()), // Should be ignored
            ..Default::default()
        };

        let result = config.get_domains();

        assert_eq!(result, vec!["player", "table"]);
    }

    /// When only domain is set, use it as fallback.
    ///
    /// Simpler env var path: ANGZARR_IPC_DOMAIN=player
    #[test]
    fn test_get_domains_falls_back_to_domain() {
        let config = IpcBusConfig {
            domains: None,
            domain: Some("player".to_string()),
            ..Default::default()
        };

        let result = config.get_domains();

        assert_eq!(result, vec!["player"]);
    }

    /// Comma-separated values in domain field are split.
    ///
    /// Env var can specify multiple: ANGZARR_IPC_DOMAIN=player,table,hand
    #[test]
    fn test_get_domains_splits_comma_separated() {
        let config = IpcBusConfig {
            domains: None,
            domain: Some("player,table,hand".to_string()),
            ..Default::default()
        };

        let result = config.get_domains();

        assert_eq!(result, vec!["player", "table", "hand"]);
    }

    /// Whitespace around commas is trimmed.
    ///
    /// User-friendly: "player, table, hand" works same as "player,table,hand"
    #[test]
    fn test_get_domains_trims_whitespace() {
        let config = IpcBusConfig {
            domains: None,
            domain: Some("player , table , hand".to_string()),
            ..Default::default()
        };

        let result = config.get_domains();

        assert_eq!(result, vec!["player", "table", "hand"]);
    }

    /// When neither domains nor domain is set, return empty vec.
    ///
    /// Publisher-only mode: no subscriptions needed.
    #[test]
    fn test_get_domains_returns_empty_when_none_set() {
        let config = IpcBusConfig {
            domains: None,
            domain: None,
            ..Default::default()
        };

        let result = config.get_domains();

        assert!(result.is_empty());
    }

    /// Empty domains vec is returned as-is (not fallen back to domain).
    ///
    /// Explicit empty array means "subscribe to nothing", not "check domain field".
    #[test]
    fn test_get_domains_empty_vec_is_explicit() {
        let config = IpcBusConfig {
            domains: Some(vec![]),
            domain: Some("player".to_string()), // Should still be ignored
            ..Default::default()
        };

        let result = config.get_domains();

        assert!(result.is_empty());
    }
}
