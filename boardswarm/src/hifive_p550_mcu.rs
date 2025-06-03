use serde::Deserialize;
use std::path::PathBuf;
use tokio_serial::SerialPortBuilderExt;
use tracing::{info, warn};

use crate::{
    registry::{self, Properties},
    serial::SerialProvider,
    udev::Device,
    Server,
};

pub const PROVIDER: &str = "hifive-p550-mcu";

pub struct HifiveP550MCUProvider {
    name: String,
    server: Server,
}

impl HifiveP550MCUProvider {
    pub fn new(name: String, server: Server) -> Self {
        Self { name, server }
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
                tokio::spawn(setup_hifive_p550_mcu(
                    node.to_path_buf(),
                    properties,
                    self.server.clone(),
                ));
                return true;
            }
        }
        false
    }

    fn remove(&mut self, _device: &Device) {
        todo!("Remove not implemented");
    }
}

async fn setup_hifive_p550_mcu(node: PathBuf, properties: Properties, server: Server) {
    info!(
        "Setting up Hifive P550 MCU serial connection for {}",
        node.display()
    );
    let port = match tokio_serial::new(node.to_string_lossy(), 115200).open_native_async() {
        Ok(port) => port,
        Err(e) => {
            warn!("Failed to open serial port: {e}");
            return;
        }
    };

    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::spawn(process(port, rx));

    let mut properties = properties.clone();
    let command = "hifive-p550-mcu-sompower";
    properties.insert(registry::NAME, command);
    server.register_actuator(
        properties,
        HifiveP550MCUCommand {
            command: command.into(),
            tx: tx.clone(),
        },
    );
}

async fn process(
    mut _port: tokio_serial::SerialStream,
    mut rx: tokio::sync::mpsc::Receiver<String>,
) {
    while let Some(command) = rx.recv().await {
        dbg!(&command);
    }
}
struct HifiveP550MCUCommand {
    command: String,
    tx: tokio::sync::mpsc::Sender<String>,
}

impl std::fmt::Debug for HifiveP550MCUCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO make more meaningful
        f.debug_struct("HifiveP550MCUCommand")
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl crate::Actuator for HifiveP550MCUCommand {
    async fn set_mode(
        &self,
        parameters: Box<dyn erased_serde::Deserializer<'static> + Send>,
    ) -> Result<(), crate::ActuatorError> {
        #[derive(Deserialize, Debug)]
        struct ModeParameters {
            value: bool,
        }
        let parameters = ModeParameters::deserialize(parameters).unwrap();
        dbg!(parameters);
        Ok(())
    }
}
