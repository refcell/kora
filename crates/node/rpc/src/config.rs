//! RPC server configuration.

use std::net::SocketAddr;

/// Configuration for the RPC server.
#[derive(Clone, Debug)]
pub struct RpcServerConfig {
    /// Address for the HTTP status endpoints.
    pub http_addr: SocketAddr,
    /// Address for the JSON-RPC server.
    pub jsonrpc_addr: SocketAddr,
    /// Chain ID for the Ethereum API.
    pub chain_id: u64,
    /// CORS configuration.
    pub cors: CorsConfig,
    /// Rate limiting configuration.
    pub rate_limit: RateLimitConfig,
    /// Maximum number of concurrent connections.
    pub max_connections: u32,
}

impl RpcServerConfig {
    /// Create a new RPC configuration with default CORS and rate limiting.
    pub fn new(http_addr: SocketAddr, jsonrpc_addr: SocketAddr, chain_id: u64) -> Self {
        Self {
            http_addr,
            jsonrpc_addr,
            chain_id,
            cors: CorsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            max_connections: 100,
        }
    }

    /// Create a configuration with the same address for both HTTP and JSON-RPC.
    pub fn single_addr(addr: SocketAddr, chain_id: u64) -> Self {
        Self::new(addr, addr, chain_id)
    }

    /// Set CORS allowed origins.
    #[must_use]
    pub fn with_cors_origins(mut self, origins: Vec<String>) -> Self {
        self.cors.allowed_origins = origins;
        self
    }

    /// Set rate limit.
    #[must_use]
    pub const fn with_rate_limit(mut self, requests_per_second: u64) -> Self {
        self.rate_limit.requests_per_second = requests_per_second;
        self
    }

    /// Set maximum connections.
    #[must_use]
    pub const fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }
}

impl Default for RpcServerConfig {
    fn default() -> Self {
        Self {
            http_addr: "127.0.0.1:8545".parse().unwrap(),
            jsonrpc_addr: "127.0.0.1:8545".parse().unwrap(),
            chain_id: 1,
            cors: CorsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            max_connections: 100,
        }
    }
}

/// CORS configuration for the RPC server.
#[derive(Clone, Debug)]
pub struct CorsConfig {
    /// Allowed origins. Empty means no CORS headers are sent.
    /// Use `["*"]` to allow all origins (not recommended for production).
    pub allowed_origins: Vec<String>,
    /// Allowed methods.
    pub allowed_methods: Vec<String>,
    /// Allowed headers.
    pub allowed_headers: Vec<String>,
    /// Max age for preflight cache (seconds).
    pub max_age: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["http://localhost:3000".to_string()],
            allowed_methods: vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()],
            allowed_headers: vec!["Content-Type".to_string()],
            max_age: 3600,
        }
    }
}

impl CorsConfig {
    /// Create a restrictive CORS config that allows no origins.
    pub const fn none() -> Self {
        Self {
            allowed_origins: Vec::new(),
            allowed_methods: Vec::new(),
            allowed_headers: Vec::new(),
            max_age: 0,
        }
    }

    /// Create a permissive CORS config for development.
    ///
    /// **Warning:** Do not use in production.
    pub fn permissive() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec!["*".to_string()],
            max_age: 86400,
        }
    }
}

/// Rate limiting configuration.
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum requests per second per client.
    pub requests_per_second: u64,
    /// Burst size for rate limiting.
    pub burst_size: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self { requests_per_second: 100, burst_size: 200 }
    }
}

impl RateLimitConfig {
    /// Disable rate limiting.
    pub const fn disabled() -> Self {
        Self { requests_per_second: u64::MAX, burst_size: u64::MAX }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_server_config_default() {
        let config = RpcServerConfig::default();
        assert_eq!(config.http_addr, "127.0.0.1:8545".parse().unwrap());
        assert_eq!(config.jsonrpc_addr, "127.0.0.1:8545".parse().unwrap());
        assert_eq!(config.chain_id, 1);
        assert_eq!(config.max_connections, 100);
    }

    #[test]
    fn rpc_server_config_new() {
        let http: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let jsonrpc: SocketAddr = "127.0.0.1:8545".parse().unwrap();
        let config = RpcServerConfig::new(http, jsonrpc, 42);

        assert_eq!(config.http_addr, http);
        assert_eq!(config.jsonrpc_addr, jsonrpc);
        assert_eq!(config.chain_id, 42);
        assert_eq!(config.max_connections, 100);
    }

    #[test]
    fn rpc_server_config_single_addr() {
        let addr: SocketAddr = "0.0.0.0:9000".parse().unwrap();
        let config = RpcServerConfig::single_addr(addr, 137);

        assert_eq!(config.http_addr, addr);
        assert_eq!(config.jsonrpc_addr, addr);
        assert_eq!(config.chain_id, 137);
    }

    #[test]
    fn rpc_server_config_with_cors_origins() {
        let config =
            RpcServerConfig::default().with_cors_origins(vec!["https://example.com".to_string()]);
        assert_eq!(config.cors.allowed_origins, vec!["https://example.com"]);
    }

    #[test]
    fn rpc_server_config_with_rate_limit() {
        let config = RpcServerConfig::default().with_rate_limit(500);
        assert_eq!(config.rate_limit.requests_per_second, 500);
    }

    #[test]
    fn rpc_server_config_with_max_connections() {
        let config = RpcServerConfig::default().with_max_connections(200);
        assert_eq!(config.max_connections, 200);
    }

    #[test]
    fn rpc_server_config_chained_builder() {
        let config = RpcServerConfig::default()
            .with_cors_origins(vec!["*".to_string()])
            .with_rate_limit(1000)
            .with_max_connections(50);

        assert_eq!(config.cors.allowed_origins, vec!["*"]);
        assert_eq!(config.rate_limit.requests_per_second, 1000);
        assert_eq!(config.max_connections, 50);
    }

    #[test]
    fn cors_config_default() {
        let config = CorsConfig::default();
        assert_eq!(config.allowed_origins, vec!["http://localhost:3000"]);
        assert_eq!(config.allowed_methods, vec!["GET", "POST", "OPTIONS"]);
        assert_eq!(config.allowed_headers, vec!["Content-Type"]);
        assert_eq!(config.max_age, 3600);
    }

    #[test]
    fn cors_config_none() {
        let config = CorsConfig::none();
        assert!(config.allowed_origins.is_empty());
        assert!(config.allowed_methods.is_empty());
        assert!(config.allowed_headers.is_empty());
        assert_eq!(config.max_age, 0);
    }

    #[test]
    fn cors_config_permissive() {
        let config = CorsConfig::permissive();
        assert_eq!(config.allowed_origins, vec!["*"]);
        assert!(config.allowed_methods.contains(&"GET".to_string()));
        assert!(config.allowed_methods.contains(&"POST".to_string()));
        assert!(config.allowed_methods.contains(&"PUT".to_string()));
        assert!(config.allowed_methods.contains(&"DELETE".to_string()));
        assert!(config.allowed_methods.contains(&"OPTIONS".to_string()));
        assert_eq!(config.allowed_headers, vec!["*"]);
        assert_eq!(config.max_age, 86400);
    }

    #[test]
    fn rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.requests_per_second, 100);
        assert_eq!(config.burst_size, 200);
    }

    #[test]
    fn rate_limit_config_disabled() {
        let config = RateLimitConfig::disabled();
        assert_eq!(config.requests_per_second, u64::MAX);
        assert_eq!(config.burst_size, u64::MAX);
    }

    #[test]
    fn rpc_server_config_clone() {
        let original = RpcServerConfig::default().with_rate_limit(250).with_max_connections(75);
        let cloned = original.clone();

        assert_eq!(cloned.rate_limit.requests_per_second, 250);
        assert_eq!(cloned.max_connections, 75);
    }

    #[test]
    fn cors_config_clone() {
        let original = CorsConfig::permissive();
        let cloned = original.clone();

        assert_eq!(cloned.allowed_origins, vec!["*"]);
        assert_eq!(cloned.max_age, 86400);
    }

    #[test]
    fn rate_limit_config_clone() {
        let original = RateLimitConfig { requests_per_second: 500, burst_size: 1000 };
        let cloned = original.clone();

        assert_eq!(cloned.requests_per_second, 500);
        assert_eq!(cloned.burst_size, 1000);
    }

    #[test]
    fn rpc_server_config_debug() {
        let config = RpcServerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("RpcServerConfig"));
        assert!(debug_str.contains("chain_id"));
    }

    #[test]
    fn cors_config_debug() {
        let config = CorsConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("CorsConfig"));
        assert!(debug_str.contains("allowed_origins"));
    }

    #[test]
    fn rate_limit_config_debug() {
        let config = RateLimitConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("RateLimitConfig"));
        assert!(debug_str.contains("requests_per_second"));
    }
}
