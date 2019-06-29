use sample::Sample;

use std::{
    borrow::BorrowMut,
    iter::Peekable,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use super::{source::Source, SampleFormat};

// it's important to note that even though we are using a vector (for cache
// locality reasons), order of our elements doesn't matter (A + B = B + A),
// so we can make optimizations like using swap_remove() instead of remove().
type Sources<'a> = Arc<Mutex<Vec<(Option<&'static str>, Peekable<Source<'a>>)>>>;

#[derive(Clone)]
pub struct Mixer<'a>(Sources<'a>);

impl<'a> Mixer<'a> {
    pub fn new() -> Self {
        Mixer(Arc::new(Mutex::new(Vec::new())))
    }

    pub fn add(&mut self, name: Option<&'static str>, input: Source<'a>) {
        self.0.lock().unwrap().push((name, input.peekable()))
    }

    pub fn remove(&mut self, name: &'static str) {
        let name = Some(name);
        swap_retain(self.0.lock().unwrap(), |(n, _)| n != &name);
    }
}

/*pub struct MixerIterator<'a>(Sources<'a>);

impl<'a> IntoIterator for Mixer<'a> {
    type Item = SampleFormat;
    type IntoIter = MixerIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        MixerIterator(self.0)
    }
}*/

impl<'a> Iterator for Mixer<'a> {
    type Item = SampleFormat;

    fn next(&mut self) -> Option<Self::Item> {
        swap_retain(self.0.lock().unwrap(), |(_, i)| i.peek().is_some());

        let mut i = 0;
        let mut accum = <Self::Item as Sample>::Signed::equilibrium();
        while let Some((_, input)) = self.0.lock().unwrap().get_mut(i) {
            accum = accum.add_amp(input.next().unwrap());
            i += 1;
        }

        Some(accum.to_sample())
    }
}

fn swap_retain<T, F: FnMut(&mut T) -> bool>(mut vec: impl DerefMut<Target = Vec<T>>, mut f: F) {
    let vec = vec.borrow_mut();
    for i in 0..vec.len() {
        if !f(&mut vec[i]) {
            vec.swap_remove(i);
        }
    }
}
