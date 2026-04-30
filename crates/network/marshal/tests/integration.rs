//! Integration tests for kora-marshal initializers.
//!
//! These tests verify that all initializers work together to start a marshal actor,
//! following the pattern from commonware-consensus tests.

#![allow(missing_docs)]
#![allow(clippy::unit_arg)]
#![allow(clippy::type_complexity)]

mod common;

use std::{
    collections::BTreeMap,
    future::Future,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};

use commonware_consensus::{
    Heightable, Reporter,
    marshal::{Update, core::Mailbox, standard::Standard},
    simplex::{
        scheme::bls12381_threshold::standard as bls12381_threshold,
        types::{Activity, Finalization, Finalize, Notarization, Notarize, Proposal},
    },
    types::{Epoch, Height, Round, View},
};
use commonware_cryptography::{
    Digestible, Hasher as _,
    bls12381::primitives::variant::MinPk,
    certificate::{ConstantProvider, Scheme as CertificateScheme, mocks::Fixture},
    ed25519::PublicKey,
    sha256::{Digest as Sha256Digest, Sha256},
};
use commonware_macros::test_traced;
use commonware_p2p::{
    Manager,
    simulated::{self, Link, Network, Oracle},
};
use commonware_parallel::Sequential;
use commonware_runtime::{Clock, Metrics, Quota, Runner, deterministic};
use commonware_utils::{Acknowledgement, NZU16, NZUsize, ordered::Set};
use kora_marshal::{ActorInitializer, ArchiveInitializer, BroadcastInitializer, PeerInitializer};

use crate::common::Block;

// Type aliases matching commonware tests
type D = Sha256Digest;
type K = PublicKey;
type V = MinPk;
type S = bls12381_threshold::Scheme<K, V>;
type B = Block;

// Test constants
const NAMESPACE: &[u8] = b"test";
const NUM_VALIDATORS: u32 = 4;
const QUORUM: u32 = 3;
const LINK: Link = Link {
    latency: Duration::from_millis(100),
    jitter: Duration::from_millis(1),
    success_rate: 1.0,
};
const TEST_QUOTA: Quota = Quota::per_second(NonZeroU32::MAX);

/// Mock application that tracks received blocks.
#[derive(Clone, Default)]
struct MockApplication {
    blocks: Arc<Mutex<BTreeMap<Height, B>>>,
    tip: Arc<Mutex<Option<(Height, <B as Digestible>::Digest)>>>,
}

impl MockApplication {
    fn blocks(&self) -> BTreeMap<Height, B> {
        self.blocks.lock().unwrap().clone()
    }
}

impl Reporter for MockApplication {
    type Activity = Update<B>;

    fn report(&mut self, activity: Self::Activity) -> impl Future<Output = ()> + Send {
        match activity {
            Update::Block(block, ack) => {
                let height = block.height();
                self.blocks.lock().unwrap().insert(height, block);
                ack.acknowledge();
            }
            Update::Tip(_, height, commitment) => {
                *self.tip.lock().unwrap() = Some((height, commitment));
            }
        }
        async {}
    }
}

/// Helper to create notarizations.
fn make_notarization(proposal: Proposal<D>, schemes: &[S], quorum: u32) -> Notarization<S, D> {
    let notarizes: Vec<_> = schemes
        .iter()
        .take(quorum as usize)
        .map(|scheme| Notarize::sign(scheme, proposal.clone()).unwrap())
        .collect();
    Notarization::from_notarizes(&schemes[0], &notarizes, &Sequential).unwrap()
}

/// Helper to create finalizations.
fn make_finalization(proposal: Proposal<D>, schemes: &[S], quorum: u32) -> Finalization<S, D> {
    let finalizes: Vec<_> = schemes
        .iter()
        .take(quorum as usize)
        .map(|scheme| Finalize::sign(scheme, proposal.clone()).unwrap())
        .collect();
    Finalization::from_finalizes(&schemes[0], &finalizes, &Sequential).unwrap()
}

/// Sets up a validator using the kora-marshal initializers.
async fn setup_validator(
    context: deterministic::Context,
    oracle: &mut Oracle<K, deterministic::Context>,
    validator: K,
    provider: ConstantProvider<S, Epoch>,
) -> (MockApplication, Mailbox<S, Standard<B>>, Height) {
    // 1. Use PeerInitializer::init() for the resolver
    let control = oracle.control(validator.clone());
    let backfill = control.register(1, TEST_QUOTA).await.unwrap();

    let resolver = PeerInitializer::init::<_, _, _, B, _, _, _>(
        &context,
        validator.clone(),
        oracle.manager(),
        control.clone(),
        backfill,
    );

    // 2. Use BroadcastInitializer::init() for the broadcast engine
    let (broadcast_engine, buffer) = BroadcastInitializer::init::<_, _, B, _>(
        context.clone(),
        validator.clone(),
        oracle.manager(),
        (),
    );
    let network = control.register(2, TEST_QUOTA).await.unwrap();
    broadcast_engine.start(network);

    // 3. Use ArchiveInitializer::init_finalizations() for finalizations archive
    let finalizations_by_height = ArchiveInitializer::init_finalizations(
        context.with_label("finalizations_by_height"),
        S::certificate_codec_config_unbounded(),
    )
    .await
    .expect("failed to init finalizations archive");

    // 4. Use ArchiveInitializer::init_blocks() for blocks archive
    let finalized_blocks =
        ArchiveInitializer::init_blocks(context.with_label("finalized_blocks"), ())
            .await
            .expect("failed to init blocks archive");

    // 5. Use ActorInitializer::init() for the actor
    let (actor, mailbox, processed_height) = ActorInitializer::init(
        context.clone(),
        finalizations_by_height,
        finalized_blocks,
        provider,
        commonware_runtime::buffer::paged::CacheRef::from_pooler(
            &context,
            NZU16!(1024),
            NZUsize!(10),
        ),
        (),
    )
    .await;

    // Create and start with mock application
    let application = MockApplication::default();
    actor.start(application.clone(), buffer, resolver);

    (application, mailbox, processed_height)
}

/// Sets up network links between all peers.
async fn setup_network_links(
    oracle: &mut Oracle<K, deterministic::Context>,
    peers: &[K],
    link: Link,
) {
    for p1 in peers.iter() {
        for p2 in peers.iter() {
            if p2 == p1 {
                continue;
            }
            let _ = oracle.add_link(p1.clone(), p2.clone(), link.clone()).await;
        }
    }
}

/// Integration test that starts a marshal actor and finalizes a block.
#[test_traced("WARN")]
fn test_start_marshal_and_finalize_block() {
    let runner = deterministic::Runner::timed(Duration::from_secs(60));
    runner.start(|mut context| async move {
        // Setup network
        let (network, mut oracle) = Network::new(
            context.with_label("network"),
            simulated::Config {
                max_size: 1024 * 1024,
                disconnect_on_block: true,
                tracked_peer_sets: NZUsize!(1),
            },
        );
        network.start();

        // Create cryptographic fixtures
        let Fixture { participants, schemes, .. } =
            bls12381_threshold::fixture::<V, _>(&mut context, NAMESPACE, NUM_VALIDATORS);

        // Setup a single validator using all initializers
        let validator = participants[0].clone();
        let (application, mut mailbox, processed_height) = setup_validator(
            context.with_label("validator_0"),
            &mut oracle,
            validator.clone(),
            ConstantProvider::new(schemes[0].clone()),
        )
        .await;

        // Verify initial state
        assert_eq!(processed_height, Height::zero());
        assert!(application.blocks().is_empty());

        // Create a block
        let parent = Sha256::hash(b"genesis");
        let block = Block::new(parent, Height::new(1), 1);
        let round = Round::new(Epoch::new(0), View::new(1));

        // Submit verified block
        mailbox.verified(round, block.clone()).await;

        // Create proposal
        let proposal = Proposal { round, parent: View::new(0), payload: block.digest() };

        // Notarize the block
        let notarization = make_notarization(proposal.clone(), &schemes, QUORUM);
        mailbox.report(Activity::Notarization(notarization)).await;

        // Finalize the block
        let finalization = make_finalization(proposal, &schemes, QUORUM);
        mailbox.report(Activity::Finalization(finalization)).await;

        // Wait for block to be delivered to application
        let mut attempts = 0;
        while application.blocks().is_empty() && attempts < 100 {
            context.sleep(Duration::from_millis(10)).await;
            attempts += 1;
        }

        // Verify block was delivered
        let blocks = application.blocks();
        assert_eq!(blocks.len(), 1, "Expected 1 block to be finalized");
        assert!(blocks.contains_key(&Height::new(1)));

        // Verify block can be retrieved from mailbox
        let retrieved =
            mailbox.get_block(Height::new(1)).await.expect("block should be retrievable");
        assert_eq!(retrieved.height(), Height::new(1));

        // Verify finalization can be retrieved
        let fin = mailbox
            .get_finalization(Height::new(1))
            .await
            .expect("finalization should be retrievable");
        assert_eq!(fin.proposal.payload, block.digest());
    });
}

/// Integration test with multiple validators that each verify their own block.
#[test_traced("WARN")]
fn test_start_marshal_multiple_validators() {
    let runner = deterministic::Runner::timed(Duration::from_secs(60));
    runner.start(|mut context| async move {
        // Setup network
        let (network, mut oracle) = Network::new(
            context.with_label("network"),
            simulated::Config {
                max_size: 1024 * 1024,
                disconnect_on_block: true,
                tracked_peer_sets: NZUsize!(3),
            },
        );
        network.start();

        // Create cryptographic fixtures
        let Fixture { participants, schemes, .. } =
            bls12381_threshold::fixture::<V, _>(&mut context, NAMESPACE, NUM_VALIDATORS);

        // Register peer set
        let mut manager = oracle.manager();
        manager.track(0, Set::from_iter_dedup(participants.clone())).await;

        // Setup multiple validators
        let mut applications = Vec::new();
        let mut mailboxes = Vec::new();

        for (i, validator) in participants.iter().take(2).enumerate() {
            let (app, mailbox, _) = setup_validator(
                context.with_label(&format!("validator_{i}")),
                &mut oracle,
                validator.clone(),
                ConstantProvider::new(schemes[i].clone()),
            )
            .await;
            applications.push(app);
            mailboxes.push(mailbox);
        }

        // Setup network links
        setup_network_links(&mut oracle, &participants[..2], LINK).await;

        // Create and finalize a block - both validators verify it locally
        let parent = Sha256::hash(b"genesis");
        let block = Block::new(parent, Height::new(1), 42);
        let round = Round::new(Epoch::new(0), View::new(1));

        // Both validators verify the block locally
        for mailbox in &mut mailboxes {
            mailbox.verified(round, block.clone()).await;
        }

        let proposal = Proposal { round, parent: View::new(0), payload: block.digest() };

        // Both validators receive notarization and finalization
        let notarization = make_notarization(proposal.clone(), &schemes, QUORUM);
        let finalization = make_finalization(proposal, &schemes, QUORUM);

        for mailbox in &mut mailboxes {
            mailbox.report(Activity::Notarization(notarization.clone())).await;
            mailbox.report(Activity::Finalization(finalization.clone())).await;
        }

        // Wait for blocks to be delivered
        let mut attempts = 0;
        while (applications[0].blocks().is_empty() || applications[1].blocks().is_empty())
            && attempts < 100
        {
            context.sleep(Duration::from_millis(10)).await;
            attempts += 1;
        }

        // Verify both validators received the block
        assert_eq!(applications[0].blocks().len(), 1);
        assert_eq!(applications[1].blocks().len(), 1);
    });
}
