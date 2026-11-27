use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use nms_glyph_reader::train::{NOT_GLYPH, SIDE};

mod cells;
mod distort;
mod list;
mod measure;
mod net;
mod train;

const INPUT: usize = SIDE * SIDE;
const CLASSES: usize = NOT_GLYPH + 1;
const EPOCHS: usize = 10;
const USAGE: &str = "Usage:
  trainer train <list.tsv> <model.bin> [epochs]
  trainer measure <list.tsv> [model.bin]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let epochs = args
        .get(3)
        .map_or(Some(EPOCHS), |epochs| epochs.parse().ok());
    match (&args[..], epochs) {
        (["train", list, model] | ["train", list, model, _], Some(epochs)) => {
            train::run(Path::new(list), Path::new(model), epochs);
        }
        (["measure", list], _) => measure::run(Path::new(list), None),
        (["measure", list, model], _) => measure::run(Path::new(list), Some(Path::new(model))),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1))
}

fn each<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads())
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        if i > 0 && i.is_multiple_of(1000) {
                            eprintln!("{i} of {}", items.len());
                        }
                        done.push((i, work(item)));
                    }
                    done
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}
