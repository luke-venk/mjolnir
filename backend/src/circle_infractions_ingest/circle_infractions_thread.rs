use super::infraction_byte_decoder;
use super::infraction_byte_decoder::InfractionState;
use crossbeam::channel::{Receiver, TrySendError, bounded};
use serialport::SerialPort;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CircleInfractionDetectionState {
    Stale,
    DetectedInfraction(u64), // timestamp of when infraction was detected in milliseconds since unix epoch
    KeepAlive,
}

const STALE_THRESHOLD_HZ: f64 = 5.0; // Hz
const CROSSBEAM_CHANNEL_CAPACITY: usize = 10;

// No unit tests for this because I'm not mocking the serialport library
// And because it is quite basic
fn find_arduino_port() -> String {
    let ports = serialport::available_ports().expect("Failed to list serial ports");
    let arduino_ports: Vec<_> = ports
        .into_iter()
        .filter(|p| {
            let name = &p.port_name;
            #[cfg(target_os = "linux")]
            {
                name.starts_with("/dev/ttyACM") || name.starts_with("/dev/ttyUSB")
            }
            #[cfg(target_os = "macos")]
            {
                name.starts_with("/dev/cu.usbmodem") || name.starts_with("/dev/cu.usbserial")
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
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

/// Finds the arduino on a serial port and begins reading in data
/// Returns a channel of CircleInfractionDetectionState updates, which will be
/// - Stale if we haven't heard from the arduino in a 5 cycles (assuming 20Hz cycles)
/// - DetectedInfraction when we detect an infraction
/// - KeepAlive so we can detect a dropped channel and exit the thread gracefully
/// Panics if the channel is full, which would indicate that the receiving end is not doing its job
pub fn begin_detecting_circle_infractions(baud: u32) -> Receiver<CircleInfractionDetectionState> {
    let arduino_port = find_arduino_port();
    let stale_timeout = Duration::from_secs_f64(1.0 / STALE_THRESHOLD_HZ);
    let mut port: Box<dyn SerialPort> = serialport::new(&arduino_port, baud)
        .timeout(stale_timeout)
        .open()
        .expect("Failed to open serial port {arduino_port}");
    let (tx, rx) = bounded::<CircleInfractionDetectionState>(CROSSBEAM_CHANNEL_CAPACITY);
    thread::spawn(move || {
        loop {
            if let Err(TrySendError::Disconnected(_)) =
                tx.try_send(CircleInfractionDetectionState::KeepAlive)
            {
                return;
            }
            let mut buf = [0u8; 1];
            match port.read(&mut buf) {
                Ok(1) => match infraction_byte_decoder::decode(buf[0]) {
                    Some(InfractionState::Infraction) => {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_millis() as u64;
                        match tx.try_send(CircleInfractionDetectionState::DetectedInfraction(
                            timestamp,
                        )) {
                            Ok(_) => {}
                            Err(TrySendError::Full(_)) => {
                                panic!("[uart] channel full, dropping stale state update");
                            }
                            Err(TrySendError::Disconnected(_)) => return,
                        }
                    }
                    Some(InfractionState::Clear) => {}
                    None => {
                        eprintln!("Received unrecognized byte: 0x{:02X}", buf[0]);
                    }
                },
                Ok(_) => {} // 0 bytes read, ignore
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    match tx.try_send(CircleInfractionDetectionState::Stale) {
                        Ok(_) => {}
                        Err(TrySendError::Full(_)) => {
                            panic!("[uart] channel full, dropping stale state update");
                        }
                        Err(TrySendError::Disconnected(_)) => return,
                    }
                }
                Err(e) => {
                    eprintln!("[uart] read error: {e} — attempting reconnect");
                    loop {
                        thread::sleep(Duration::from_secs(1));
                        match serialport::new(&arduino_port, baud)
                            .timeout(stale_timeout)
                            .open()
                        {
                            Ok(new_port) => {
                                port = new_port;
                                eprintln!("[uart] reconnected");
                                break;
                            }
                            Err(_) => match tx.try_send(CircleInfractionDetectionState::Stale) {
                                Ok(_) => {}
                                Err(TrySendError::Full(_)) => {
                                    panic!("[uart] channel full, what is the server doing???");
                                }
                                Err(TrySendError::Disconnected(_)) => return,
                            },
                        }
                    }
                }
            };
        }
    });
    rx
}
