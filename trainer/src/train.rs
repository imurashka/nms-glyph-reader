use std::path::Path;
use std::time::Instant;

use crate::cells::{self, Cells};
use crate::distort::distort;
use crate::net::{Adam, Net, Random, Training};
use crate::{CLASSES, INPUT, threads};

const HIDDEN: usize = 128;
const BATCH: usize = 256;
const RATE: f32 = 1e-3;

pub fn run(list: &Path, model: &Path, epochs: usize) {
    let started = Instant::now();
    let cells = cells::load(list, &model.with_extension("cells"));
    println!(
        "{} cells in {:.0} s, per class 0-F and not a glyph: {:?}",
        cells.len(),
        started.elapsed().as_secs_f64(),
        cells.per_class()
    );
    let mut random = Random::new(7);
    let mut net = Net::new(&[INPUT, HIDDEN, CLASSES], &mut random);
    let mut adam = Adam::new(&net, RATE);
    let mut order: Vec<usize> = (0..cells.len()).collect();
    let steps = epochs * cells.len().div_ceil(BATCH);
    let mut step = 0;
    for epoch in 1..=epochs {
        let begun = Instant::now();
        random.shuffle(&mut order);
        let mut loss = 0.0;
        for batch in order.chunks(BATCH) {
            let progress = step as f32 / steps as f32;
            adam.set_rate(RATE * 0.5 * (1.0 + (std::f32::consts::PI * progress).cos()));
            step += 1;
            let (mut gradient, batch_loss) = gradient(&net, &cells, batch, random.bits());
            loss += batch_loss;
            adam.apply(&mut net, &mut gradient, 1.0 / batch.len() as f32);
        }
        net.save(model);
        println!(
            "epoch {epoch}: loss {:.4}, {:.0} s",
            loss / cells.len() as f32,
            begun.elapsed().as_secs_f64()
        );
    }
    println!(
        "{} written in {:.0} s",
        model.display(),
        started.elapsed().as_secs_f64()
    );
}

fn gradient(net: &Net, cells: &Cells, batch: &[usize], seed: u64) -> (Net, f32) {
    let part = batch.len().div_ceil(threads());
    let parts: Vec<(Net, f32)> = std::thread::scope(|scope| {
        let workers: Vec<_> = batch
            .chunks(part)
            .enumerate()
            .map(|(t, slice)| {
                scope.spawn(move || {
                    let mut random = Random::new(seed ^ ((t as u64 + 1) << 32));
                    let mut gradient = net.zeros_like();
                    let mut loss = 0.0;
                    for &i in slice {
                        let input = distort(cells.input(i), &mut random);
                        loss += net.add_gradient(&input, cells.label(i), &mut gradient);
                    }
                    (gradient, loss)
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    let mut parts = parts.into_iter();
    let (mut total, mut loss) = parts.next().unwrap();
    for (gradient, part_loss) in parts {
        total.add(&gradient);
        loss += part_loss;
    }
    (total, loss)
}
