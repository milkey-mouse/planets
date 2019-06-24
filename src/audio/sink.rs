use cpal::{default_output_device, ChannelCount, Format, SampleRate};
use sample::{
    conv::ToSample,
    frame::Frame,
    signal::{IntoInterleavedSamples, Signal},
    Sample,
};

use super::{music, MASTER_SAMPLE_RATE};

pub fn test() {
    let device = cpal::default_output_device().expect("failed to find a default output device");
    let format = Format {
        channels: 2,
        sample_rate: SampleRate(MASTER_SAMPLE_RATE),
        ..device.default_output_format().unwrap()
    };
    let event_loop = cpal::EventLoop::new();
    let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
    event_loop.play_stream(stream_id.clone());

    let mut source = music::vlem().into_interleaved_samples();

    event_loop.run(move |_, data| match data {
        cpal::StreamData::Output {
            buffer: cpal::UnknownTypeOutputBuffer::U16(mut buffer),
        } => fill_stream_buffer(&mut source, &mut buffer, format.channels),
        cpal::StreamData::Output {
            buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer),
        } => fill_stream_buffer(&mut source, &mut buffer, format.channels),
        cpal::StreamData::Output {
            buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer),
        } => fill_stream_buffer(&mut source, &mut buffer, format.channels),
        _ => (),
    });
}

fn fill_stream_buffer<I, S, O>(
    source: &mut IntoInterleavedSamples<I>,
    buffer: &mut [O],
    channels: ChannelCount,
) where
    I: Signal,
    I::Frame: Frame<Sample = S>,
    S: Sample + ToSample<O>,
{
    buffer
        .chunks_mut(channels as usize)
        .flat_map(|s| s.iter_mut())
        .for_each(|s| *s = source.next_sample().to_sample());
}
