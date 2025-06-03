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

async fn setup_serial_command(
    node: PathBuf,
    properties: Properties,
    parameters: SerialCommandParameters,
    server: Server,
) {
    info!("Setting up serial cmd for {}", node.display());
    let port = match tokio_serial::new(node.to_string_lossy(), parameters.rate).open_native_async()
    {
        Ok(port) => port,
        Err(e) => {
            warn!("Failed to open serial port: {e}");
            return;
        }
    };

    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::spawn(process(port, rx));

    for command in parameters.commands {
        let mut properties = properties.clone();
        properties.insert(registry::NAME, command.name.clone());
        server.register_actuator(
            properties,
            SerialCommand {
                command,
                tx: tx.clone(),
            },
        );
    }
}

async fn process(
    mut port: tokio_serial::SerialStream,
    mut rx: tokio::sync::mpsc::Receiver<Command>,
) {
    while let Some(command) = rx.recv().await {
        let buf = format!("{}\n", command.message);
        debug!("Writing serial command: {}", &buf);
        let _ = port.write_all(buf.as_bytes());
    }
}
struct SerialCommand {
    command: Command,
    tx: tokio::sync::mpsc::Sender<Command>,
}

impl std::fmt::Debug for SerialCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO make more meaningful
        f.debug_struct("SerialCommand").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl crate::Actuator for SerialCommand {
    async fn set_mode(
        &self,
        _parameters: Box<dyn erased_serde::Deserializer<'static> + Send>,
    ) -> Result<(), crate::ActuatorError> {
        self.tx.send(self.command.clone()).await.unwrap();
        Ok(())
    }
}
