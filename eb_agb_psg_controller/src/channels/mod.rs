pub mod noise;
pub mod square;
pub mod wave;

use eb_agb_psg_interop::EnvelopeSpec;

pub fn envelope_bits(envelope: &EnvelopeSpec) -> u16 {
    ((envelope.initial_volume as u16) << 4)
        | ((envelope.increasing as u16) << 3)
        | envelope.step_time as u16
}
