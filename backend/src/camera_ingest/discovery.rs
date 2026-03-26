use aravis::Aravis;

// Information we want to print for each discovered camera.
#[derive(Debug, Clone)]
pub struct DiscoveredCamera {
    // Camera/device ID used by Aravis to identify the device.
    pub id: String,
    // Network address reported by Aravis discovery.
    pub address: String,
    pub physical_id: String,
    pub vendor: String,
    pub model: String,
    pub protocol: String,
}

/// Discover all cameras currently visible to Aravis.
pub fn discover_cameras(aravis: &Aravis) -> Vec<DiscoveredCamera> {
    aravis
        .get_device_list()
        .into_iter()
        .map(|device| DiscoveredCamera {

            id: device.id.to_string_lossy().into_owned(),
            address: device.address.to_string_lossy().into_owned(),
            physical_id: device.physical_id.to_string_lossy().into_owned(),
            vendor: device.vendor.to_string_lossy().into_owned(),
            model: device.model.to_string_lossy().into_owned(),
            protocol: device.protocol.to_string_lossy().into_owned(),
        })
        .collect()
}

// Print discovered cameras in a single-line CLI-friendly format.
pub fn print_discovered_cameras(aravis: &Aravis) {
    let cameras = discover_cameras(aravis);

    if cameras.is_empty() {
        println!("No cameras discovered.");
        return;
    }

    for camera in cameras {
        println!(
            "id={} ip={} physical_id={} vendor={} model={} protocol={}",
            camera.id,
            camera.address,
            camera.physical_id,
            camera.vendor,
            camera.model,
            camera.protocol,
        );
    }
}