use bytes::BytesMut;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex as AsyncMutex,
};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, info, warn};

use crate::{
    registry::{self, Properties},
    serial::SerialProvider,
    udev::Device,
    Server,
};

pub const PROVIDER: &str = "hifive-p550-mcu";

#[derive(Deserialize, Clone, Debug, Default)]
struct HifiveP550MCUParameters {
    #[serde(rename = "match")]
    match_: HashMap<String, String>,
}

pub struct HifiveP550MCUProvider {
    name: String,
    parameters: HifiveP550MCUParameters,
    server: Server,
}

impl HifiveP550MCUProvider {
    pub fn new(name: String, parameters: serde_yaml::Value, server: Server) -> Self {
        let parameters: HifiveP550MCUParameters = serde_yaml::from_value(parameters).unwrap();
        Self {
            name,
            parameters,
            server,
        }
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
                if !properties.matches(&self.parameters.match_) {
                    debug!(
                        "Ignoring device {} - {:?}",
                        device.syspath().display(),
                        properties,
                    );
                    return false;
                }
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

    let mut properties = properties.clone();
    let name = "hifive-p550-mcu-sompower";
    properties.insert(registry::NAME, name);
    server.register_actuator(
        properties,
        HifiveP550MCUActuator {
            name: name.into(),
            port: Arc::new(AsyncMutex::new(port)),
        },
    );
}

#[derive(Deserialize)]
struct ParametersSomPower {
    value: bool,
}

struct HifiveP550MCUActuator {
    name: String,
    port: Arc<AsyncMutex<SerialStream>>,
}

impl std::fmt::Debug for HifiveP550MCUActuator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO make more meaningful
        f.debug_struct("HifiveP550MCUActuator")
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl crate::Actuator for HifiveP550MCUActuator {
    async fn set_mode(
        &self,
        parameters: Box<dyn erased_serde::Deserializer<'static> + Send>,
    ) -> Result<(), crate::ActuatorError> {
        match self.name.as_str() {
            "hifive-p550-mcu-sompower" => {
                let parameters = ParametersSomPower::deserialize(parameters).unwrap();
                let buf = format!("sompower-s {}\n", parameters.value as usize);
                let mut port = self.port.lock().await;
                debug!("Writing buffer: {:?}", &buf);
                port.write_all(buf.as_bytes()).await.unwrap();
                let mut data = BytesMut::zeroed(1024);
                let r = port.read(&mut data).await.unwrap();
                data.truncate(r);
                debug!("Read buffer: {:?}", &data);
            }
            _ => return Err(crate::ActuatorError {}),
        };
        Ok(())
    }
}
