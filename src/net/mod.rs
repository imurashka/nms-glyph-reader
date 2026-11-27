use std::sync::OnceLock;

pub mod cell;
pub mod dot;

const GLYPHS_MODEL: &[u8] = include_bytes!("../assets/glyphs.bin");
const MAGIC: &[u8; 4] = b"NMSG";
const INPUT_SCALE: f32 = cell::SIDE as f32;

pub const NOT_GLYPH: usize = cell::DIGITS.len();

#[derive(Clone)]
pub struct Dense {
    pub inputs: usize,
    pub weights: Vec<f32>,
    pub biases: Vec<f32>,
}

impl Dense {
    pub fn zeros(inputs: usize, outputs: usize) -> Self {
        Self {
            inputs,
            weights: vec![0.0; inputs * outputs],
            biases: vec![0.0; outputs],
        }
    }

    fn apply(&self, input: &[f32]) -> Vec<f32> {
        self.weights
            .chunks_exact(self.inputs)
            .zip(&self.biases)
            .map(|(row, bias)| bias + dot::product(row, input))
            .collect()
    }
}

#[derive(Clone)]
pub struct Net {
    pub layers: Vec<Dense>,
}

impl Net {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut words = bytes
            .strip_prefix(MAGIC)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w));
        let count = words.next()? as usize;
        let mut layers = Vec::with_capacity(count);
        for _ in 0..count {
            let (inputs, outputs) = (words.next()? as usize, words.next()? as usize);
            let mut layer = Dense::zeros(inputs, outputs);
            for v in layer.weights.iter_mut().chain(layer.biases.iter_mut()) {
                *v = f32::from_bits(words.next()?);
            }
            layers.push(layer);
        }
        Some(Self { layers })
    }

    #[cfg(feature = "train")]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend((self.layers.len() as u32).to_le_bytes());
        for layer in &self.layers {
            out.extend((layer.inputs as u32).to_le_bytes());
            out.extend((layer.biases.len() as u32).to_le_bytes());
            for v in layer.weights.iter().chain(&layer.biases) {
                out.extend(v.to_le_bytes());
            }
        }
        out
    }

    pub fn activations(&self, input: &[f32]) -> Vec<Vec<f32>> {
        let mut out = vec![input.iter().map(|v| v * INPUT_SCALE).collect::<Vec<f32>>()];
        for (i, layer) in self.layers.iter().enumerate() {
            let mut next = layer.apply(out.last().unwrap());
            if i + 1 < self.layers.len() {
                next.iter_mut().for_each(|v| *v = v.max(0.0));
            }
            out.push(next);
        }
        out
    }

    pub fn probabilities(&self, input: &[f32]) -> Vec<f32> {
        softmax(self.activations(input).last().unwrap())
    }
}

pub fn softmax(logits: &[f32]) -> Vec<f32> {
    let top = logits.iter().fold(f32::MIN, |m, &v| m.max(v));
    let exps: Vec<f32> = logits.iter().map(|v| (v - top).exp()).collect();
    let sum: f32 = exps.iter().sum();
    exps.iter().map(|e| e / sum).collect()
}

static NET: OnceLock<Net> = OnceLock::new();

pub fn net() -> &'static Net {
    NET.get_or_init(|| Net::from_bytes(GLYPHS_MODEL).unwrap())
}

#[cfg(feature = "train")]
pub fn use_net(net: Net) {
    assert!(NET.set(net).is_ok(), "a glyph model is already in use");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_model_maps_a_cell_to_sixteen_digits_and_not_a_glyph() {
        let sizes: Vec<(usize, usize)> = net()
            .layers
            .iter()
            .map(|l| (l.inputs, l.biases.len()))
            .collect();
        assert_eq!(sizes.first().unwrap().0, cell::SIDE * cell::SIDE);
        assert_eq!(sizes.last().unwrap().1, NOT_GLYPH + 1);
        assert!(sizes.windows(2).all(|p| p[0].1 == p[1].0));
    }

    #[test]
    fn a_cut_model_file_reads_as_nothing() {
        assert!(Net::from_bytes(&GLYPHS_MODEL[..GLYPHS_MODEL.len() - 4]).is_none());
        assert!(Net::from_bytes(b"NMSX").is_none());
    }
}
