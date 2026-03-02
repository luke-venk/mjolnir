/**
 * All logic specific to the type of throwing event can live here. The 4
 * types of throwing events are shot put, discus throw, hammer throw, and
 * javelin throw.
 */
use serde::Serialize;

#[derive(Serialize)]
pub enum EventType {
    Shotput,
    Discus,
    Hammer,
    Javelin,
}