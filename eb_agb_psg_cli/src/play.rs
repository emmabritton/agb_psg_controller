use std::path::Path;

use eb_agb_psg_controller::{Player, Sfx, SfxChannel, host};

use crate::apu::{Apu, SAMPLE_RATE};

pub fn run(sfx: &Sfx) -> Result<(), String> {
    let samples = render(sfx);
    let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
    println!("playing ({seconds:.2}s)");
    play_samples(samples)
}

pub fn render_wav(sfx: &Sfx, path: &Path) -> Result<f32, String> {
    let samples = render(sfx);
    write_wav(path, &samples)?;
    Ok(samples.len() as f32 / SAMPLE_RATE as f32)
}

fn render(sfx: &Sfx) -> Vec<f32> {
    let channel = match sfx.channel {
        SfxChannel::SquareSweep => 0,
        SfxChannel::Square => 1,
        SfxChannel::Wave => 2,
        SfxChannel::Noise => 3,
    };
    host::reset();
    let mut player = Player::new();
    player.play_sfx(sfx);
    let mut apu = Apu::new();
    let mut samples = Vec::new();
    let samples_per_frame = SAMPLE_RATE as f64 * 280_896.0 / 16_777_216.0;
    let mut acc = 0.0f64;
    let max_samples = SAMPLE_RATE as usize * 60;
    while samples.len() < max_samples {
        player.frame();
        apu.apply(&host::take_writes());
        acc += samples_per_frame;
        let count = acc as usize;
        acc -= count as f64;
        apu.render(&mut samples, count);
        if !player.debug_sfx_active(channel) {
            break;
        }
    }
    apu.render(&mut samples, SAMPLE_RATE as usize / 5);
    samples
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(value).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

fn play_samples(samples: Vec<f32>) -> Result<(), String> {
    let (_stream, handle) =
        rodio::OutputStream::try_default().map_err(|e| format!("audio output: {e}"))?;
    let sink = rodio::Sink::try_new(&handle).map_err(|e| format!("audio output: {e}"))?;
    sink.append(rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, samples));
    sink.sleep_until_end();
    Ok(())
}
