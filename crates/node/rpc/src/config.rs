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
    pub fn with_cors_origins(mut self, origins: Vec<String>) -> Self {
        self.cors.allowed_origins = origins;
        self
    }

    /// Set rate limit.
    pub fn with_rate_limit(mut self, requests_per_second: u64) -> Self {
        self.rate_limit.requests_per_second = requests_per_second;
        self
    }

    /// Set maximum connections.
    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
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
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec!["Content-Type".to_string()],
            max_age: 3600,
        }
    }
}

impl CorsConfig {
    /// Create a restrictive CORS config that allows no origins.
    pub fn none() -> Self {
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
        Self {
            requests_per_second: 100,
            burst_size: 200,
        }
    }
}

impl RateLimitConfig {
    /// Disable rate limiting.
    pub fn disabled() -> Self {
        Self {
            requests_per_second: u64::MAX,
            burst_size: u64::MAX,
        }
    }
}
