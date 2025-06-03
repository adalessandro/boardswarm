use std::path::PathBuf;

use tokio_serial::SerialPortBuilderExt;
use tracing::{info, warn};

use crate::{registry, serial::SerialProvider, udev::Device};

pub const PROVIDER: &str = "hifive-p550-mcu";

pub struct HifiveP550MCUProvider {
    name: String,
}

impl HifiveP550MCUProvider {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl SerialProvider for HifiveP550MCUProvider {
    fn handle(&mut self, device: &crate::udev::Device, _seqnum: u64) -> bool {
        let provider_properties = &[
            (registry::PROVIDER_NAME, self.name.as_str()),
            (registry::PROVIDER, PROVIDER),
        ];
        if device.property_u64("ID_VENDOR_ID", 16) != Some(0x0403) {
            return false;
        };
        if device.property_u64("ID_MODEL_ID", 16) != Some(0x6001) {
            return false;
        };

        if let Some(node) = device.devnode() {
            if let Some(name) = node.file_name() {
                let mut properties = device.properties(name.to_string_lossy());
                properties.extend(provider_properties);
                tokio::spawn(setup_volume(node.to_path_buf()));

                return true;
            }
        }
        false
    }

    fn remove(&mut self, _device: &Device) {}
}

async fn setup_volume(node: PathBuf) {
    info!("Setting up brom volume for {}", node.display());
    let port = match tokio_serial::new(node.to_string_lossy(), 115200).open_native_async() {
        Ok(port) => port,
        Err(e) => {
            warn!("Failed to open serial port: {e}");
            return;
        }
    };
    dbg!(port);
}
