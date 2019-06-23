use cpal::{Format, SampleRate};
use sample::{
    signal::{Signal},
    Sample,
};


use std::thread;

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

    let source = music::vlem().into_interleaved_samples().into_iter();

    let (sender, receiver): (
        std::sync::mpsc::SyncSender<f64>,
        std::sync::mpsc::Receiver<f64>,
    ) = std::sync::mpsc::sync_channel(128);

    thread::spawn(move || {
        event_loop.run(move |_, data| match data {
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::U16(mut buffer),
            } => {
                for sample in buffer.chunks_mut(format.channels as usize) {
                    for out in sample.iter_mut() {
                        *out = receiver.recv().unwrap().to_sample::<u16>();
                    }
                }
            }
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer),
            } => {
                for sample in buffer.chunks_mut(format.channels as usize) {
                    for out in sample.iter_mut() {
                        *out = receiver.recv().unwrap().to_sample::<i16>();
                    }
                }
            }
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer),
            } => {
                for sample in buffer.chunks_mut(format.channels as usize) {
                    for out in sample.iter_mut() {
                        *out = receiver.recv().unwrap().to_sample::<f32>();
                    }
                }
            }
            _ => (),
        })
    });

    for sample in source {
        sender.send(sample).unwrap();
    }
}
