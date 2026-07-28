//! Stateless register drivers for the four PSG channels. All playback state
//! lives in the player; these functions just write configurations to hardware.

pub mod noise;
pub mod square;
pub mod wave;

use eb_agb_psg_interop::EnvelopeSpec;

/// NRx2-layout envelope byte: bits 4-7 initial volume, bit 3 direction, bits 0-2 step time.
pub fn envelope_bits(envelope: &EnvelopeSpec) -> u16 {
    ((envelope.initial_volume as u16) << 4)
        | ((envelope.increasing as u16) << 3)
        | envelope.step_time as u16
}
