use sample::{frame::Frame, interpolate, ring_buffer, signal::Signal};

pub mod music;
pub mod sink;
mod source;

// TODO: dynamically set master sample rate to rate of open sink
const MASTER_SAMPLE_RATE: u32 = 44100;
const HIGH_QUALITY_INTERPOLATION: bool = true;
//const SINC_INITIALIZATION_ARRAY: [f64; 100];
// type SampleFormat = f64;

fn resample<'a, F: Frame<Sample = f64> + 'static>(
    in_rate: u32,
    out_rate: Option<u32>,
    mut signal: Box<dyn Signal<Frame = F>>,
) -> Box<dyn Signal<Frame = F>> {
    let out_rate = out_rate.unwrap_or(MASTER_SAMPLE_RATE);

    if in_rate == out_rate {
        signal
    } else if HIGH_QUALITY_INTERPOLATION {
        let buffer = ring_buffer::Fixed::from([F::equilibrium(); 100]);
        let sinc = interpolate::Sinc::new(buffer);
        Box::new(signal.from_hz_to_hz(sinc, in_rate.into(), out_rate.into()))
    } else {
        let linear = interpolate::Linear::from_source(&mut signal);
        Box::new(signal.from_hz_to_hz(linear, in_rate.into(), out_rate.into()))
    }
}
