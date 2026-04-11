#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InfractionState {
    Clear = 0x00,      // 0000_0000
    Infraction = 0xFE, // 1111_1110
}

/// Maximum bit-error distance we are willing to correct.
/// 0x00 and 0xFE are 7 bits apart, so distance <= 2 means
/// the byte is unambiguously closer to one state than the other.
const MAX_CORRECT_DISTANCE: u32 = 2;

fn hamming(a: u8, b: u8) -> u32 {
    (a ^ b).count_ones()
}

/// Try to decode a raw byte into an InfractionState.
/// Returns None if the byte is too ambiguous to correct.
pub fn decode(raw: u8) -> Option<InfractionState> {
    if raw == InfractionState::Clear as u8 {
        return Some(InfractionState::Clear);
    }
    if raw == InfractionState::Infraction as u8 {
        return Some(InfractionState::Infraction);
    }

    let d_clear = hamming(raw, InfractionState::Clear as u8);
    let d_infraction = hamming(raw, InfractionState::Infraction as u8);

    if d_clear < d_infraction && d_clear <= MAX_CORRECT_DISTANCE {
        Some(InfractionState::Clear)
    } else if d_infraction < d_clear && d_infraction <= MAX_CORRECT_DISTANCE {
        Some(InfractionState::Infraction)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0x00, Some(InfractionState::Clear))]
    #[case(0xFE, Some(InfractionState::Infraction))]
    #[case(0x01, Some(InfractionState::Clear))]
    #[case(0xFD, Some(InfractionState::Infraction))]
    #[case(0x70, None)]
    fn test_decode(#[case] input: u8, #[case] expected: Option<InfractionState>) {
        assert_eq!(decode(input), expected);
    }
}
