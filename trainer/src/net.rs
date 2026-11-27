use std::path::Path;

pub use nms_glyph_reader::train::Net;
use nms_glyph_reader::train::{Dense, softmax};

pub struct Random(u64);

impl Random {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn bits(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn unit(&mut self) -> f32 {
        (self.bits() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn normal(&mut self) -> f32 {
        let (a, b) = (self.unit().max(1e-7), self.unit());
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, (self.bits() % (i as u64 + 1)) as usize);
        }
    }
}

pub trait Training: Sized {
    fn new(sizes: &[usize], random: &mut Random) -> Self;
    fn zeros_like(&self) -> Self;
    fn add_gradient(&self, input: &[f32], label: usize, into: &mut Self) -> f32;
    fn add(&mut self, other: &Self);
    fn values_mut(&mut self) -> impl Iterator<Item = &mut f32>;
    fn load(path: &Path) -> Self;
    fn save(&self, path: &Path);
}

impl Training for Net {
    fn new(sizes: &[usize], random: &mut Random) -> Self {
        let layers = sizes
            .windows(2)
            .map(|pair| {
                let mut layer = Dense::zeros(pair[0], pair[1]);
                let spread = (2.0 / pair[0] as f32).sqrt();
                layer
                    .weights
                    .iter_mut()
                    .for_each(|w| *w = random.normal() * spread);
                layer
            })
            .collect();
        Self { layers }
    }

    fn zeros_like(&self) -> Self {
        Self {
            layers: self
                .layers
                .iter()
                .map(|l| Dense::zeros(l.inputs, l.biases.len()))
                .collect(),
        }
    }

    fn add_gradient(&self, input: &[f32], label: usize, into: &mut Self) -> f32 {
        let activations = self.activations(input);
        let mut delta = softmax(activations.last().unwrap());
        let loss = -delta[label].max(1e-12).ln();
        delta[label] -= 1.0;
        for (i, layer) in self.layers.iter().enumerate().rev() {
            let before = &activations[i];
            let grad = &mut into.layers[i];
            for (o, d) in delta.iter().enumerate() {
                grad.biases[o] += d;
                let row = &mut grad.weights[o * layer.inputs..(o + 1) * layer.inputs];
                row.iter_mut().zip(before).for_each(|(g, x)| *g += d * x);
            }
            if i > 0 {
                let mut back = vec![0.0; layer.inputs];
                for (o, d) in delta.iter().enumerate() {
                    let row = &layer.weights[o * layer.inputs..(o + 1) * layer.inputs];
                    back.iter_mut().zip(row).for_each(|(b, w)| *b += d * w);
                }
                back.iter_mut().zip(before).for_each(|(b, x)| {
                    if *x <= 0.0 {
                        *b = 0.0;
                    }
                });
                delta = back;
            }
        }
        loss
    }

    fn add(&mut self, other: &Self) {
        for (a, b) in self.layers.iter_mut().zip(&other.layers) {
            a.weights
                .iter_mut()
                .zip(&b.weights)
                .for_each(|(x, y)| *x += y);
            a.biases
                .iter_mut()
                .zip(&b.biases)
                .for_each(|(x, y)| *x += y);
        }
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut f32> {
        self.layers
            .iter_mut()
            .flat_map(|l| l.weights.iter_mut().chain(l.biases.iter_mut()))
    }

    fn load(path: &Path) -> Self {
        Self::from_bytes(&std::fs::read(path).unwrap()).unwrap()
    }

    fn save(&self, path: &Path) {
        std::fs::write(path, self.to_bytes()).unwrap();
    }
}

pub struct Adam {
    rate: f32,
    step: i32,
    first: Net,
    second: Net,
}

impl Adam {
    pub fn new(net: &Net, rate: f32) -> Self {
        Self {
            rate,
            step: 0,
            first: net.zeros_like(),
            second: net.zeros_like(),
        }
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
    }

    pub fn apply(&mut self, net: &mut Net, gradient: &mut Net, scale: f32) {
        self.step += 1;
        let (b1, b2) = (0.9f32, 0.999f32);
        let correction = (1.0 - b2.powi(self.step)).sqrt() / (1.0 - b1.powi(self.step));
        let rate = self.rate * correction;
        for (((value, g), m), v) in net
            .values_mut()
            .zip(gradient.values_mut())
            .zip(self.first.values_mut())
            .zip(self.second.values_mut())
        {
            let g = *g * scale;
            *m = b1 * *m + (1.0 - b1) * g;
            *v = b2 * *v + (1.0 - b2) * g * g;
            *value -= rate * *m / (v.sqrt() + 1e-8);
        }
    }
}
