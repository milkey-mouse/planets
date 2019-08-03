use cpal::{
    platform::{Device, StreamId},
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleRate, StreamData, StreamDataResult, SupportedFormat, UnknownTypeOutputBuffer,
};
use crossbeam_utils::thread::{scope, Scope};
use sample::{conv::ToSample, Sample};

use std::{
    convert::TryInto,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use super::{mixer::Mixer, source::Source, Channels, SampleFormat};
use crate::util::IntentionalPanic;

pub trait Sink<'a> {
    fn play(&mut self, name: Option<&'static str>, source: Source<'a>);
    fn play_singleton(&mut self, name: &'static str, source: Source<'a>);

    fn channels(&self) -> Option<Channels>;
    fn sample_rate(&self) -> Option<NonZeroU32>;
}

struct DummySink;

impl<'a> Sink<'a> for DummySink {
    fn play(&mut self, _name: Option<&'static str>, _source: Source<'a>) {}
    fn play_singleton(&mut self, _name: &'static str, _source: Source<'a>) {}

    fn channels(&self) -> Option<Channels> {
        None
    }
    fn sample_rate(&self) -> Option<NonZeroU32> {
        None
    }
}

#[derive(Clone)]
pub struct AudioThread<'a> {
    mixer: Mixer<'a>,
    format: Format,
    stopping: Arc<AtomicBool>,
}

impl<'a> Sink<'a> for AudioThread<'a> {
    fn play(&mut self, name: Option<&'static str>, source: Source<'a>) {
        self.mixer.add(name, source);
    }

    fn play_singleton(&mut self, name: &'static str, source: Source<'a>) {
        self.mixer.remove(name);
        self.mixer.add(Some(name), source);
    }

    fn channels(&self) -> Option<Channels> {
        self.format.channels.try_into().ok()
    }

    fn sample_rate(&self) -> Option<NonZeroU32> {
        Some(NonZeroU32::new(self.format.sample_rate.0).unwrap())
    }
}

impl<'a> Drop for AudioThread<'a> {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
    }
}

impl<'a> AudioThread<'a> {
    pub fn with<F: FnOnce(Box<dyn Sink<'a> + 'a>) + 'a>(f: F) {
        // TODO: allow cpal::EventLoop::run() to terminate
        // here we have to write a custom panic hander(!) because the audio thread has to panic in
        // order to exit at all from event_loop.run().
        IntentionalPanic::setup_hook();
        scope(|s| f(Self::new(s))).unwrap_err();
    }

    fn new(scope: &Scope<'a>) -> Box<dyn Sink<'a> + 'a> {
        match Self::spawn(scope) {
            Ok(real) => Box::new(real),
            Err(_) => Box::new(DummySink {}),
        }
    }

    fn spawn(scope: &Scope<'a>) -> Result<Self, ()> {
        let host = cpal::default_host();
        // TODO: sound device selection menu
        // see issue #2
        let device = host.default_output_device().ok_or(())?;
        let format = Self::get_output_format(&device)?;

        let event_loop = host.event_loop();
        let stream_id = event_loop
            .build_output_stream(&device, &format)
            .or(Err(()))?;
        event_loop.play_stream(stream_id);

        let sink = Self {
            mixer: Mixer::new(),
            stopping: Arc::new(AtomicBool::new(false)),
            format,
        };

        let mut audio_thread = sink.clone();
        scope.spawn(move |_| event_loop.run(move |id, data| audio_thread.callback(id, data)));

        Ok(sink)
    }

    fn get_output_format(device: &Device) -> Result<Format, ()> {
        const HZ_44100: Option<SampleRate> = Some(SampleRate(44100));

        match device
            .supported_output_formats()
            .ok()
            .and_then(|s| s.max_by(SupportedFormat::cmp_default_heuristics))
        {
            Some(SupportedFormat {
                channels,
                min_sample_rate,
                max_sample_rate,
                data_type,
            }) => Some(Format {
                channels,
                sample_rate: HZ_44100
                    .filter(|r| (min_sample_rate..=max_sample_rate).contains(r))
                    .unwrap_or(max_sample_rate),
                data_type,
            }),
            None => device.default_output_format().ok(),
        }
        .filter(|f| f.channels <= 2)
        .ok_or(())
    }

    fn callback(&mut self, _id: StreamId, data: StreamDataResult) {
        match data.unwrap() {
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::U16(mut buffer),
            } => self.fill_stream_buffer(&mut buffer),
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::I16(mut buffer),
            } => self.fill_stream_buffer(&mut buffer),
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::F32(mut buffer),
            } => self.fill_stream_buffer(&mut buffer),
            _ => (),
        }

        if self.stopping.load(Ordering::Acquire) {
            panic!(IntentionalPanic); // this is the only way to end this thread, since event_loop.run won't return
        }
    }

    fn fill_stream_buffer<O>(&mut self, buffer: &mut [O])
    where
        O: Sample,
        SampleFormat: Sample + ToSample<O>,
    {
        // NOTE: it would not be correct to directly copy interleaved samples
        // instead of doing it on a frame-by-frame basis were it not for the
        // implementation of source::new, which dynamically ensures the frame
        // width is the same as the sink's (by doubling mono or mixing stereo).
        for sample in buffer {
            *sample = self
                .mixer
                .next()
                .map(Sample::to_sample)
                .unwrap_or_else(O::equilibrium);
        }
    }
}
