//! Execution configuration.

use revm::primitives::hardfork::SpecId;

/// Gas limit bounds for block validation.
#[derive(Clone, Debug)]
pub struct GasLimitBounds {
    /// Minimum gas limit.
    pub min: u64,
    /// Maximum gas limit.
    pub max: u64,
    /// Maximum change from parent (denominator for delta calculation).
    /// Gas limit can change by at most parent_gas_limit / max_delta_divisor.
    pub max_delta_divisor: u64,
}

impl GasLimitBounds {
    /// Default gas limit bounds.
    pub const DEFAULT: Self = Self { min: 5000, max: u64::MAX, max_delta_divisor: 1024 };
}

impl Default for GasLimitBounds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// EIP-1559 base fee calculation parameters.
#[derive(Clone, Debug)]
pub struct BaseFeeParams {
    /// Elasticity multiplier (default: 2).
    pub elasticity_multiplier: u64,
    /// Base fee max change denominator (default: 8).
    pub max_change_denominator: u64,
}

impl BaseFeeParams {
    /// Default base fee parameters.
    pub const DEFAULT: Self = Self { elasticity_multiplier: 2, max_change_denominator: 8 };
}

impl Default for BaseFeeParams {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Execution configuration.
#[derive(Clone, Debug)]
pub struct ExecutionConfig {
    /// Chain ID for transaction validation.
    pub chain_id: u64,
    /// Hardfork specification.
    pub spec_id: SpecId,
    /// Gas limit bounds.
    pub gas_limit_bounds: GasLimitBounds,
    /// EIP-1559 base fee parameters.
    pub base_fee_params: BaseFeeParams,
}

impl ExecutionConfig {
    /// Create a new execution config with the given chain ID.
    pub const fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            spec_id: SpecId::CANCUN,
            gas_limit_bounds: GasLimitBounds::DEFAULT,
            base_fee_params: BaseFeeParams::DEFAULT,
        }
    }

    /// Set the hardfork specification.
    #[must_use]
    pub const fn with_spec_id(mut self, spec_id: SpecId) -> Self {
        self.spec_id = spec_id;
        self
    }

    /// Set the gas limit bounds.
    #[must_use]
    pub const fn with_gas_limit_bounds(mut self, bounds: GasLimitBounds) -> Self {
        self.gas_limit_bounds = bounds;
        self
    }

    /// Set the base fee parameters.
    #[must_use]
    pub const fn with_base_fee_params(mut self, params: BaseFeeParams) -> Self {
        self.base_fee_params = params;
        self
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default() {
        let config = ExecutionConfig::default();
        assert_eq!(config.chain_id, 1);
        assert_eq!(config.spec_id, SpecId::CANCUN);
    }

    #[test]
    fn config_builder() {
        let config = ExecutionConfig::new(42).with_spec_id(SpecId::PRAGUE).with_gas_limit_bounds(
            GasLimitBounds { min: 10000, max: 30_000_000, max_delta_divisor: 512 },
        );

        assert_eq!(config.chain_id, 42);
        assert_eq!(config.spec_id, SpecId::PRAGUE);
        assert_eq!(config.gas_limit_bounds.min, 10000);
        assert_eq!(config.gas_limit_bounds.max_delta_divisor, 512);
    }

    #[test]
    fn gas_limit_bounds_default() {
        let bounds = GasLimitBounds::default();
        assert_eq!(bounds.min, 5000);
        assert_eq!(bounds.max, u64::MAX);
        assert_eq!(bounds.max_delta_divisor, 1024);
    }

    #[test]
    fn base_fee_params_default() {
        let params = BaseFeeParams::default();
        assert_eq!(params.elasticity_multiplier, 2);
        assert_eq!(params.max_change_denominator, 8);
    }
}
