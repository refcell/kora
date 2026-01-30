//! Interactive DKG ceremony runner.
//!
//! This module orchestrates the full DKG ceremony using the protocol and network modules.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, info, warn};

use crate::{
    DkgConfig, DkgError, DkgOutput, DkgPhase, PersistedDkgState,
    network::DkgNetwork,
    protocol::{DkgParticipant, ProtocolMessage, ProtocolMessageKind},
};

/// Phase 2 max timeout (collecting dealer messages and sending acks).
const PHASE2_MAX_TIMEOUT_SECS: u64 = 120;
/// Phase 4 max timeout (collecting dealer logs for finalization).
const PHASE4_MAX_TIMEOUT_SECS: u64 = 120;
/// Initial backoff delay for retries.
const INITIAL_BACKOFF_MS: u64 = 100;
/// Maximum backoff delay for retries.
const MAX_BACKOFF_MS: u64 = 5000;
/// Progress log interval.
const PROGRESS_LOG_INTERVAL_SECS: u64 = 5;

/// DKG ceremony runner.
pub struct DkgCeremony {
    config: DkgConfig,
    force_restart: bool,
}

impl DkgCeremony {
    /// Create a new DKG ceremony.
    pub const fn new(config: DkgConfig) -> Self {
        Self { config, force_restart: false }
    }

    /// Create a new DKG ceremony with force restart (ignores existing state).
    pub const fn new_with_force_restart(config: DkgConfig, force_restart: bool) -> Self {
        Self { config, force_restart }
    }

    /// Run the interactive DKG ceremony.
    pub async fn run(&self) -> Result<DkgOutput, DkgError> {
        info!(
            validator_index = self.config.validator_index,
            n = self.config.n(),
            t = self.config.t(),
            is_leader = self.is_leader(),
            force_restart = self.force_restart,
            "Starting interactive DKG ceremony"
        );

        // Check if we already have output
        if DkgOutput::exists(&self.config.data_dir) {
            info!("DKG output already exists, loading from disk");
            return DkgOutput::load(&self.config.data_dir);
        }

        // Clear state if force restart is requested
        if self.force_restart && PersistedDkgState::exists(&self.config.data_dir) {
            info!("Force restart requested, clearing existing state");
            PersistedDkgState::clear(&self.config.data_dir)?;
        }

        // Initialize network
        let network = DkgNetwork::new(self.config.clone())?;

        // Wait for peers to be ready
        self.wait_for_peers(&network).await?;

        // Generate a deterministic timestamp for the ceremony.
        // In production, this should be coordinated via the leader or a shared clock.
        // For now, we round down to 5-minute intervals for coordination tolerance.
        let timestamp_nanos = {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
            let interval = 5 * 60 * 1_000_000_000u64; // 5 minutes in nanos
            (now / interval) * interval
        };

        // Try to restore from persisted state, or create new participant
        let (mut participant, restored_phase) =
            match DkgParticipant::try_restore(&self.config, timestamp_nanos)? {
                Some(p) => {
                    let phase = p.current_phase();
                    info!(phase = %phase, "Restored DKG participant from persisted state");
                    (p, Some(phase))
                }
                None => {
                    let p = DkgParticipant::new(self.config.clone(), timestamp_nanos)?;
                    (p, None)
                }
            };

        // Determine starting phase based on restored state
        let skip_phase1 = matches!(
            restored_phase,
            Some(DkgPhase::DealerStarted)
                | Some(DkgPhase::CollectingMessages)
                | Some(DkgPhase::DealerFinalized)
                | Some(DkgPhase::CollectingLogs)
        );
        let skip_phase3 = matches!(
            restored_phase,
            Some(DkgPhase::DealerFinalized) | Some(DkgPhase::CollectingLogs)
        );

        // Phase 1: Start dealer - generate and broadcast commitments/shares
        if !skip_phase1 {
            info!("Phase 1: Starting dealer - generating and broadcasting commitments/shares");
            participant.start_dealer()?;
            participant.set_phase(DkgPhase::DealerStarted);
            participant.save_state(&self.config.data_dir)?;
            self.send_outgoing(&network, &mut participant)?;
        } else {
            info!("Phase 1: Skipped (restored from state)");
        }

        // Phase 2: Collect dealer messages and send acks
        participant.set_phase(DkgPhase::CollectingMessages);
        participant.save_state(&self.config.data_dir)?;
        self.run_phase2(&network, &mut participant).await?;

        // Phase 2.5: Wait for all nodes to be ready (ensures acks are received)
        if !skip_phase3 {
            self.run_phase2_ready(&network, &mut participant).await?;
        }

        // Phase 3: Finalize our dealer (only after all nodes have exchanged acks)
        if !skip_phase3 {
            self.run_phase3(&network, &mut participant)?;
            participant.set_phase(DkgPhase::DealerFinalized);
            participant.save_state(&self.config.data_dir)?;
        } else {
            info!("Phase 3: Skipped (restored from state)");
        }

        // Phase 4: Collect dealer logs
        participant.set_phase(DkgPhase::CollectingLogs);
        participant.save_state(&self.config.data_dir)?;
        self.run_phase4(&network, &mut participant).await?;

        // Save state periodically during phase 4 (handled in run_phase4)
        participant.save_state(&self.config.data_dir)?;

        // Phase 5: Finalize and produce output
        info!("Phase 5: Finalizing DKG");
        let output = match participant.finalize() {
            Ok(o) => o,
            Err(e) => {
                participant.set_phase(DkgPhase::Failed);
                participant.save_state(&self.config.data_dir)?;
                return Err(e);
            }
        };

        // Mark as completed and clear state file
        participant.set_phase(DkgPhase::Completed);
        PersistedDkgState::clear(&self.config.data_dir)?;

        // Save output
        output.save(&self.config.data_dir)?;
        info!(
            share_index = output.share_index,
            path = ?self.config.data_dir,
            "DKG ceremony completed and saved"
        );

        Ok(output)
    }

    /// Phase 2: Collect dealer messages from quorum and send acks.
    async fn run_phase2(
        &self,
        network: &DkgNetwork,
        participant: &mut DkgParticipant,
    ) -> Result<(), DkgError> {
        info!(
            required = participant.required_quorum(),
            total = participant.total_participants(),
            "Phase 2: Collecting dealer messages and sending acks"
        );

        let deadline = Instant::now() + Duration::from_secs(PHASE2_MAX_TIMEOUT_SECS);
        let mut last_progress_log = Instant::now();
        let mut backoff = ExponentialBackoff::new();

        while Instant::now() < deadline {
            self.receive_and_process(network, participant)?;
            self.send_outgoing(network, participant)?;

            let received = participant.received_dealer_count();
            let acks_sent = participant.acks_sent_count();
            let required = participant.required_quorum();
            let total = participant.total_participants();

            // Log progress periodically
            if last_progress_log.elapsed() >= Duration::from_secs(PROGRESS_LOG_INTERVAL_SECS) {
                info!(
                    received,
                    acks_sent,
                    required,
                    total,
                    "Phase 2 progress: received {}/{} dealer messages, sent {}/{} acks",
                    received,
                    total,
                    acks_sent,
                    total
                );
                last_progress_log = Instant::now();
            }

            // Phase 2 completes when we have quorum dealer messages AND have sent acks
            if received >= required && acks_sent >= required {
                info!(
                    received,
                    acks_sent,
                    required,
                    "Phase 2 complete: received quorum dealer messages and sent acks"
                );
                return Ok(());
            }

            tokio::time::sleep(backoff.next_delay()).await;
        }

        Err(DkgError::Timeout)
    }

    /// Phase 2.5: Wait for all nodes to signal ready.
    ///
    /// This ensures all acks have been delivered before any dealer finalizes,
    /// preventing the race condition where a dealer finalizes with incomplete acks.
    async fn run_phase2_ready(
        &self,
        network: &DkgNetwork,
        participant: &mut DkgParticipant,
    ) -> Result<(), DkgError> {
        info!(
            total = participant.total_participants(),
            "Phase 2.5: Broadcasting ready and waiting for all participants"
        );

        // Broadcast our ready signal
        participant.broadcast_ready();
        self.send_outgoing(network, participant)?;

        let deadline = Instant::now() + Duration::from_secs(PHASE2_MAX_TIMEOUT_SECS);
        let mut last_progress_log = Instant::now();
        let mut backoff = ExponentialBackoff::new();

        while Instant::now() < deadline {
            self.receive_and_process(network, participant)?;
            self.send_outgoing(network, participant)?;

            let ready = participant.ready_count();
            let total = participant.total_participants();

            // Log progress periodically
            if last_progress_log.elapsed() >= Duration::from_secs(PROGRESS_LOG_INTERVAL_SECS) {
                info!(ready, total, "Phase 2.5 progress: {}/{} ready", ready, total);
                last_progress_log = Instant::now();
            }

            // Phase 2.5 completes when all participants are ready
            if participant.all_ready() {
                info!(ready, total, "Phase 2.5 complete: all participants ready");
                return Ok(());
            }

            tokio::time::sleep(backoff.next_delay()).await;
        }

        Err(DkgError::CeremonyFailed(format!(
            "Phase 2.5 timeout: only {}/{} participants ready",
            participant.ready_count(),
            participant.total_participants()
        )))
    }

    /// Phase 3: Finalize our dealer.
    fn run_phase3(
        &self,
        network: &DkgNetwork,
        participant: &mut DkgParticipant,
    ) -> Result<(), DkgError> {
        info!("Phase 3: Finalizing dealer and broadcasting log");
        participant.finalize_dealer()?;
        self.send_outgoing(network, participant)?;

        Ok(())
    }

    /// Phase 4: Collect dealer logs until we can finalize.
    async fn run_phase4(
        &self,
        network: &DkgNetwork,
        participant: &mut DkgParticipant,
    ) -> Result<(), DkgError> {
        info!(
            required = participant.required_quorum(),
            "Phase 4: Collecting dealer logs for finalization"
        );

        let deadline = Instant::now() + Duration::from_secs(PHASE4_MAX_TIMEOUT_SECS);
        let mut last_progress_log = Instant::now();
        let mut last_request_time = Instant::now() - Duration::from_secs(10); // Allow immediate first request
        let mut backoff = ExponentialBackoff::new();

        while Instant::now() < deadline {
            self.receive_and_process(network, participant)?;
            self.send_outgoing(network, participant)?;

            let logs = participant.dealer_log_count();
            let required = participant.required_quorum();

            // Log progress periodically
            if last_progress_log.elapsed() >= Duration::from_secs(PROGRESS_LOG_INTERVAL_SECS) {
                info!(
                    logs,
                    required, "Phase 4 progress: collected {}/{} dealer logs", logs, required
                );
                last_progress_log = Instant::now();
            }

            // Check if we can finalize
            if participant.can_finalize() {
                info!(logs, required, "Phase 4 complete: collected enough dealer logs");
                return Ok(());
            }

            // Request logs from leader if we don't have enough and haven't requested recently
            if !self.is_leader()
                && logs < required
                && last_request_time.elapsed() >= Duration::from_secs(5)
                && let Some(leader_pk) = self.config.participants.first()
            {
                debug!(logs, required, "Requesting logs from leader");
                let request_msg = ProtocolMessage::new(
                    participant.ceremony_id(),
                    ProtocolMessageKind::RequestLogs,
                );
                let _ = network.send_to(leader_pk, &request_msg);
                last_request_time = Instant::now();
            }

            tokio::time::sleep(backoff.next_delay()).await;
        }

        Err(DkgError::CeremonyFailed(format!(
            "Phase 4 timeout: only collected {}/{} dealer logs",
            participant.dealer_log_count(),
            participant.required_quorum()
        )))
    }

    /// Check if this node is the leader (coordinator).
    const fn is_leader(&self) -> bool {
        self.config.validator_index == 0
    }

    /// Wait for peers to be reachable.
    async fn wait_for_peers(&self, _network: &DkgNetwork) -> Result<(), DkgError> {
        info!("Waiting for peers to be ready...");

        let start = Instant::now();

        // Give leader time to start first
        if !self.is_leader() {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // For non-bootstrap nodes, wait for network initialization
        if !self.config.bootstrap_peers.is_empty() {
            // Simple delay to allow network setup rather than legacy ping
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        info!(elapsed = ?start.elapsed(), "Peer initialization complete");
        Ok(())
    }

    /// Send all outgoing messages.
    fn send_outgoing(
        &self,
        network: &DkgNetwork,
        participant: &mut DkgParticipant,
    ) -> Result<(), DkgError> {
        for (target, msg) in participant.take_outgoing() {
            match target {
                Some(pk) => {
                    if let Err(e) = network.send_to(&pk, &msg) {
                        debug!(?pk, ?e, "Failed to send to peer");
                    }
                }
                None => {
                    if let Err(e) = network.broadcast(&msg) {
                        debug!(?e, "Failed to broadcast");
                    }
                }
            }
        }
        Ok(())
    }

    /// Receive and process incoming messages.
    fn receive_and_process(
        &self,
        network: &DkgNetwork,
        participant: &mut DkgParticipant,
    ) -> Result<(), DkgError> {
        for envelope in network.poll_incoming() {
            if let Err(e) = participant.handle_message_bytes(&envelope.from, &envelope.payload) {
                warn!(?e, "Failed to handle message");
            }
        }
        Ok(())
    }
}

/// Exponential backoff helper for retry delays.
struct ExponentialBackoff {
    current_ms: u64,
}

impl ExponentialBackoff {
    const fn new() -> Self {
        Self { current_ms: INITIAL_BACKOFF_MS }
    }

    fn next_delay(&mut self) -> Duration {
        let delay = Duration::from_millis(self.current_ms);
        self.current_ms = (self.current_ms * 2).min(MAX_BACKOFF_MS);
        delay
    }
}
