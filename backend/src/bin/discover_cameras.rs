// Tool for users to discover and print out the specifications of
// cameras on their network via the command-line.
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::discovery::*;
use backend_lib::timing::init_global_time;

pub fn main() {
    init_global_time();
    println!("--------------------------------------------");
    println!("DISCOVERING CAMERAS ON LOCAL AREA NETWORK...");
    println!("--------------------------------------------");
    let aravis = initialize_aravis();
    let cameras = discover_cameras(&aravis);
    print_discovered_cameras(&cameras);
    println!(
        "\nWhen using the recording tool, be sure to use the camera name in the command line argument."
    );
    println!("Example: --camera \"Lucid Vision Labs-ATP124S-M-224300917\"\n")
}
