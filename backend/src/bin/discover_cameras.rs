use aravis::Aravis;
use mjolnir::camera_ingest::discovery::discover_cameras;

fn main() {
    let aravis = Aravis::initialize().expect("failed to initialize Aravis");
    let cameras = discover_cameras(&aravis);

    if cameras.is_empty() {
        println!("No cameras discovered.");
        return;
    }

    for cam in cameras {
        println!(
            "id={} ip={} physical_id={} vendor={} model={} protocol={}",
            cam.id,
            cam.address,
            cam.physical_id,
            cam.vendor,
            cam.model,
            cam.protocol,
        );
    }
}