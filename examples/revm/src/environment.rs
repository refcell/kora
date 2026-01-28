//! Simulation environment wiring for the example chain.

use commonware_cryptography::ed25519;
use commonware_p2p::simulated;
use commonware_runtime::tokio;

use crate::application::{NodeEnvironment, TransportContext, TransportControl};

pub(crate) type SimTransport = simulated::Oracle<ed25519::PublicKey, TransportContext>;

fn transport_control(
    transport: &SimTransport,
    me: ed25519::PublicKey,
) -> simulated::Control<ed25519::PublicKey, TransportContext> {
    simulated::Oracle::control(transport, me)
}

fn transport_manager(
    transport: &SimTransport,
) -> simulated::Manager<ed25519::PublicKey, TransportContext> {
    simulated::Oracle::manager(transport)
}

pub(crate) struct SimEnvironment<'a> {
    context: tokio::Context,
    transport: &'a mut SimTransport,
}

impl<'a> SimEnvironment<'a> {
    pub(crate) const fn new(context: tokio::Context, transport: &'a mut SimTransport) -> Self {
        Self { context, transport }
    }
}

impl TransportControl for SimTransport {
    type Control = simulated::Control<ed25519::PublicKey, TransportContext>;
    type Manager = simulated::Manager<ed25519::PublicKey, TransportContext>;

    fn control(&self, me: ed25519::PublicKey) -> Self::Control {
        transport_control(self, me)
    }

    fn manager(&self) -> Self::Manager {
        transport_manager(self)
    }
}

impl NodeEnvironment for SimEnvironment<'_> {
    type Transport = SimTransport;

    fn context(&self) -> tokio::Context {
        self.context.clone()
    }

    fn transport(&mut self) -> &mut SimTransport {
        self.transport
    }
}
