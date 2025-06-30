use std::{collections::HashMap, net::SocketAddr, time::Duration};

use naia_shared::metrics::RollingWindow;

pub struct BandwidthMonitor {
    total_monitor: RollingWindow,
    client_monitors: HashMap<SocketAddr, RollingWindow>,
    bandwidth_measure_duration: Duration,
}

const BYTES_TO_KBPS_FACTOR: f32 = 0.008;

impl BandwidthMonitor {
    pub fn new(bandwidth_measure_duration: Duration) -> Self {
        BandwidthMonitor {
            bandwidth_measure_duration,
            total_monitor: RollingWindow::new(bandwidth_measure_duration),
            client_monitors: HashMap::new(),
        }
    }

    pub fn create_client(&mut self, address: &SocketAddr) {
        self.client_monitors.insert(
            *address,
            RollingWindow::new(self.bandwidth_measure_duration),
        );
    }

    pub fn delete_client(&mut self, address: &SocketAddr) {
        self.client_monitors.remove(address);
    }

    pub fn record_packet(&mut self, address: &SocketAddr, bytes: usize) {
        if let Some(client_monitor) = self.client_monitors.get_mut(address) {
            client_monitor.sample(bytes as f32 * BYTES_TO_KBPS_FACTOR);

            self.total_monitor.sample(bytes as f32 * BYTES_TO_KBPS_FACTOR);
        }
    }

    pub fn total_bandwidth(&mut self) -> f32 {
        self.total_monitor.mean()
    }

    pub fn client_bandwidth(&mut self, address: &SocketAddr) -> f32 {
        self.client_monitors
            .get_mut(address)
            .expect("client associated with address does not exist")
            .mean()
    }
}
