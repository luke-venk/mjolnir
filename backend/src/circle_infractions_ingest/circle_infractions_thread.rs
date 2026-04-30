use crossbeam::channel::{bounded, Receiver, TrySendError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::time::ptp_offset;
use super::infraction_byte_decoder;
use super::infraction_byte_decoder::InfractionState;

#[derive(Debug, Clone)]
pub struct CircleInfractionTimestamps {
    pub local_arrival_ns: u64,
    pub approx_ptp_ns: Option<u64>,
    pub raw_byte: u8,
}

#[derive(Debug, Clone)]
pub enum CircleInfractionDetectionState {
    KeepAlive,
    DetectedInfraction(CircleInfractionTimestamps),
    Stale,
}

const CAPACITY: usize = 32;

fn local_now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(feature = "real")]
fn find_arduino_port() -> String {
    let ports = serialport::available_ports().expect("Failed to list serial ports");
    let arduino_ports: Vec<_> = ports
        .into_iter()
        .filter(|p| {
            let name = &p.port_name;
            #[cfg(target_os = "macos")]
            {
                name.starts_with("/dev/cu.usbmodem") || name.starts_with("/dev/cu.usbserial")
            }
            #[cfg(target_os = "linux")]
            {
                name.starts_with("/dev/ttyACM") || name.starts_with("/dev/ttyUSB")
            }
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            {
                false
            }
        })
        .collect();

    match arduino_ports.as_slice() {
        [one] => {
            println!("Found Arduino port: {}", one.port_name);
            one.port_name.to_owned()
        }
        [] => panic!("No Arduino port found"),
        ports => panic!(
            "Multiple Arduino ports found, expected exactly one: {:?}",
            ports.iter().map(|p| &p.port_name).collect::<Vec<_>>()
        ),
    }
}

#[cfg(feature = "real")]
pub fn begin_detecting_circle_infractions(baud: u32) -> Receiver<CircleInfractionDetectionState> {
    let arduino_port = find_arduino_port();

    // 5 Hz staleness threshold
    let timeout = Duration::from_millis(200);

    let mut port = serialport::new(&arduino_port, baud)
        .timeout(timeout)
        .open()
        .unwrap_or_else(|e| panic!("Failed to open serial port {}: {}", arduino_port, e));

    let (tx, rx) = bounded::<CircleInfractionDetectionState>(CAPACITY);

    thread::spawn(move || loop {
        if let Err(TrySendError::Disconnected(_)) = tx.try_send(CircleInfractionDetectionState::KeepAlive) {
            return;
        }

        let mut buf = [0u8; 1];
        match port.read(&mut buf) {
            Ok(1) => {
                match infraction_byte_decoder::decode(buf[0]) {
                    Some(InfractionState::Infraction) => {
                        let local_arrival_ns = local_now_ns();
                        let approx_ptp_ns = ptp_offset::estimate_ptp_ns(local_arrival_ns);

                        let info = CircleInfractionTimestamps {
                            local_arrival_ns,
                            approx_ptp_ns,
                            raw_byte: buf[0],
                        };

                        let _ = tx.try_send(CircleInfractionDetectionState::DetectedInfraction(info));
                    }
                    Some(InfractionState::Clear) => {
                        // ignore clears
                    }
                    None => {
                        // ignore unknown bytes
                    }
                }
            }
            Ok(_) => {}
            Err(_) => {
                // timeout or other read error: mark stale
                let _ = tx.try_send(CircleInfractionDetectionState::Stale);
            }
        }
    });

    rx
}

#[cfg(not(feature = "real"))]
pub fn begin_detecting_circle_infractions(_baud: u32) -> Receiver<CircleInfractionDetectionState> {
    let (tx, rx) = bounded::<CircleInfractionDetectionState>(CAPACITY);

    thread::spawn(move || loop {
        if let Err(TrySendError::Disconnected(_)) = tx.try_send(CircleInfractionDetectionState::KeepAlive) {
            return;
        }

        // Fake mode: simulate an occasional infraction (about 1 per ~30s)
        thread::sleep(Duration::from_millis(50));

        if rand::random::<f64>() < (1.0 / 600.0) {
            let local_arrival_ns = local_now_ns();
            let info = CircleInfractionTimestamps {
                local_arrival_ns,
                approx_ptp_ns: None,
                raw_byte: 0xFE,
            };
            let _ = tx.try_send(CircleInfractionDetectionState::DetectedInfraction(info));
        }
    });

    rx
}
