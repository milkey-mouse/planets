use sample::{frame::Frame, signal::Signal, Sample};

// it's important to note that even though we are using a vector (for cache
// locality reasons), order of our elements doesn't matter (A + B = B + A),
// so we can make optimizations like using swap_remove() instead of remove().
pub struct Mixer<F: Frame + 'static>(
    Vec<(
        Option<&'static str>,
        Box<dyn Signal<Frame = F> + Send + Sync>,
    )>,
);

impl<F: Frame + 'static> Mixer<F> {
    pub fn new() -> Self {
        Mixer(Vec::new())
    }

    pub fn add(
        &mut self,
        name: Option<&'static str>,
        signal: Box<dyn Signal<Frame = F> + Send + Sync>,
    ) {
        self.0.push((name, Box::new(signal)))
    }

    pub fn remove(&mut self, name: &'static str) {
        let name = Some(name);
        swap_retain(&mut self.0, |(n, _)| n != &name);
    }
}

impl<F: Frame + 'static> Signal for Mixer<F> {
    type Frame = F;

    fn next(&mut self) -> Self::Frame {
        swap_retain(&mut self.0, |(_, signal)| !signal.is_exhausted());
        self.0
            .iter_mut()
            .map(|(_, s)| s)
            .fold(F::Signed::equilibrium(), |f, s| {
                f.add_amp(s.next().to_signed_frame())
            })
            .map(Sample::to_sample)
    }

    fn is_exhausted(&self) -> bool {
        //self.values().any(Signal::is_exhausted)
        false
    }
}

fn swap_retain<T, F: FnMut(&T) -> bool>(vec: &mut Vec<T>, mut f: F) {
    for i in 0..vec.len() {
        if !f(&vec[i]) {
            vec.swap_remove(i);
        }
    }
}
