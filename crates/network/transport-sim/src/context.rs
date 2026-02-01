//! Simulated transport context wrapper.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, SystemTime},
};

use commonware_runtime::{self, tokio};
use governor::clock::{Clock as GovernorClock, ReasonablyRealtime};
use prometheus_client::registry::Metric;
use rand::{RngCore, rngs::OsRng};

const PORT_BASE_MIN: u16 = 40_000;
const PORT_BASE_MAX: u16 = 65_535 - 1_024;

fn remap_socket(socket: SocketAddr, port_offset: u16) -> SocketAddr {
    let port = socket.port();
    if port >= 1024 {
        return socket;
    }
    let remapped = port + port_offset;
    match socket.ip() {
        IpAddr::V4(ip) => SocketAddr::new(IpAddr::V4(ip), remapped),
        IpAddr::V6(ip) => SocketAddr::new(IpAddr::V6(ip), remapped),
    }
}

/// Tokio context wrapper for simulated networking.
///
/// Forces binding to localhost with randomized port offsets to allow
/// multiple simulated nodes to run in the same process without port conflicts.
pub struct SimContext {
    inner: tokio::Context,
    force_base_addr: bool,
    port_offset: u16,
}

impl fmt::Debug for SimContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimContext")
            .field("port_offset", &self.port_offset)
            .field("force_base_addr", &self.force_base_addr)
            .finish_non_exhaustive()
    }
}

impl SimContext {
    /// Create a new simulation context wrapping a tokio context.
    pub fn new(inner: tokio::Context) -> Self {
        let mut rng = OsRng;
        let span = u32::from(PORT_BASE_MAX - PORT_BASE_MIN + 1);
        let base = PORT_BASE_MIN + (rng.next_u32() % span) as u16;
        Self { inner, force_base_addr: true, port_offset: base }
    }
}

impl Clone for SimContext {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), force_base_addr: false, port_offset: self.port_offset }
    }
}

impl GovernorClock for SimContext {
    type Instant = SystemTime;

    fn now(&self) -> Self::Instant {
        <tokio::Context as GovernorClock>::now(&self.inner)
    }
}

impl ReasonablyRealtime for SimContext {}

impl commonware_runtime::Clock for SimContext {
    fn current(&self) -> SystemTime {
        self.inner.current()
    }

    fn sleep(&self, duration: Duration) -> impl std::future::Future<Output = ()> + Send + 'static {
        self.inner.sleep(duration)
    }

    fn sleep_until(
        &self,
        deadline: SystemTime,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        self.inner.sleep_until(deadline)
    }
}

impl commonware_runtime::Metrics for SimContext {
    fn label(&self) -> String {
        self.inner.label()
    }

    fn with_label(&self, label: &str) -> Self {
        Self {
            inner: self.inner.with_label(label),
            force_base_addr: false,
            port_offset: self.port_offset,
        }
    }

    fn register<N: Into<String>, H: Into<String>>(&self, name: N, help: H, metric: impl Metric) {
        self.inner.register(name, help, metric);
    }

    fn encode(&self) -> String {
        self.inner.encode()
    }
}

impl commonware_runtime::Spawner for SimContext {
    fn shared(mut self, blocking: bool) -> Self {
        self.inner = self.inner.shared(blocking);
        self
    }

    fn dedicated(mut self) -> Self {
        self.inner = self.inner.dedicated();
        self
    }

    fn instrumented(mut self) -> Self {
        self.inner = self.inner.instrumented();
        self
    }

    fn spawn<F, Fut, T>(self, f: F) -> commonware_runtime::Handle<T>
    where
        F: FnOnce(Self) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let port_offset = self.port_offset;
        self.inner.spawn(move |context| {
            let context = SimContext { inner: context, force_base_addr: false, port_offset };
            f(context)
        })
    }

    fn stop(
        self,
        value: i32,
        timeout: Option<Duration>,
    ) -> impl std::future::Future<Output = Result<(), commonware_runtime::Error>> + Send {
        self.inner.stop(value, timeout)
    }

    fn stopped(&self) -> commonware_runtime::signal::Signal {
        self.inner.stopped()
    }
}

impl commonware_runtime::Network for SimContext {
    type Listener = <tokio::Context as commonware_runtime::Network>::Listener;

    fn bind(
        &self,
        socket: SocketAddr,
    ) -> impl std::future::Future<Output = Result<Self::Listener, commonware_runtime::Error>> + Send
    {
        self.inner.bind(remap_socket(socket, self.port_offset))
    }

    fn dial(
        &self,
        socket: SocketAddr,
    ) -> impl std::future::Future<
        Output = Result<
            (commonware_runtime::SinkOf<Self>, commonware_runtime::StreamOf<Self>),
            commonware_runtime::Error,
        >,
    > + Send {
        self.inner.dial(remap_socket(socket, self.port_offset))
    }
}

impl RngCore for SimContext {
    fn next_u32(&mut self) -> u32 {
        if self.force_base_addr {
            self.force_base_addr = false;
            return u32::from(Ipv4Addr::LOCALHOST);
        }
        let mut rng = OsRng;
        RngCore::next_u32(&mut rng)
    }

    fn next_u64(&mut self) -> u64 {
        let mut rng = OsRng;
        RngCore::next_u64(&mut rng)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut rng = OsRng;
        RngCore::fill_bytes(&mut rng, dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        let mut rng = OsRng;
        RngCore::try_fill_bytes(&mut rng, dest)
    }
}
