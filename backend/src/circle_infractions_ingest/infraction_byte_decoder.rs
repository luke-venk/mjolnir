#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfractionState {
    Clear,
    Infraction,
}

pub fn decode(b: u8) -> Option<InfractionState> {
    match b {
        0x01 => Some(InfractionState::Clear),
        0xFE => Some(InfractionState::Infraction),
        _ => None,
    }
}
