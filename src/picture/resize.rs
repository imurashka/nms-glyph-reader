const NEGLIGIBLE: f64 = 1e-3;

struct Share {
    to: usize,
    from: usize,
    weight: f32,
}

fn shares(from_count: usize, to_count: usize) -> Vec<Share> {
    let covers = 1.0 / (to_count as f64 / from_count as f64);
    let mut shares = Vec::new();
    for to in 0..to_count {
        let start = to as f64 * covers;
        let end = start + covers;
        let covered = covers.min(from_count as f64 - start);
        let last = (end.floor() as usize).min(from_count - 1);
        let first = (start.ceil() as usize).min(last);
        if first as f64 - start > NEGLIGIBLE {
            shares.push(Share {
                to,
                from: first - 1,
                weight: ((first as f64 - start) / covered) as f32,
            });
        }
        for from in first..last {
            shares.push(Share {
                to,
                from,
                weight: (1.0 / covered) as f32,
            });
        }
        if end - last as f64 > NEGLIGIBLE {
            shares.push(Share {
                to,
                from: last,
                weight: ((end - last as f64).min(1.0).min(covered) / covered) as f32,
            });
        }
    }
    shares
}

fn by_area<T: Copy + Into<f32>>(
    source: &[T],
    size: (usize, usize),
    to: (usize, usize),
    channels: usize,
) -> Vec<f32> {
    let (width, height) = size;
    let (to_width, to_height) = to;
    let across = shares(width, to_width);
    let down = shares(height, to_height);
    let to_line = to_width * channels;
    let mut out = vec![0.0; to_line * to_height];
    let mut line = vec![0.0f32; width * channels];
    let mut narrowed = vec![0.0f32; to_line];
    let mut gathered = vec![0.0f32; to_line];
    let mut gathering = down[0].to;
    for share in &down {
        let source_line = &source[share.from * width * channels..][..width * channels];
        let mut i = 0;
        while i < line.len() {
            line[i] = source_line[i].into();
            i += 1;
        }
        narrowed.fill(0.0);
        let mut k = 0;
        while k < across.len() {
            let x = &across[k];
            let mut c = 0;
            while c < channels {
                narrowed[x.to * channels + c] += line[x.from * channels + c] * x.weight;
                c += 1;
            }
            k += 1;
        }
        if share.to != gathering {
            out[gathering * to_line..][..to_line].copy_from_slice(&gathered);
            gathered.fill(0.0);
            gathering = share.to;
        }
        let mut i = 0;
        while i < to_line {
            gathered[i] += share.weight * narrowed[i];
            i += 1;
        }
    }
    out[gathering * to_line..][..to_line].copy_from_slice(&gathered);
    out
}

pub fn shrink(rgb: &[u8], size: (usize, usize), to: (usize, usize)) -> Vec<u8> {
    by_area(rgb, size, to, 3)
        .into_iter()
        .map(|v| v.round_ties_even().clamp(0.0, 255.0) as u8)
        .collect()
}

fn enlarged(source: &[f32], side: usize, to: usize) -> Vec<f32> {
    let ratio = to as f64 / side as f64;
    let taps: Vec<(usize, f32)> = (0..to)
        .map(|i| {
            let from = (i as f64 / ratio).floor() as usize;
            let next = ((i + 1) as f64 - (from + 1) as f64 * ratio) as f32;
            let next = if next <= 0.0 {
                0.0
            } else {
                next - next.floor()
            };
            (from, next)
        })
        .collect();
    let widened: Vec<Vec<f32>> = source
        .chunks(side)
        .map(|line| {
            taps.iter()
                .map(|&(from, next)| {
                    if from >= side - 1 {
                        line[side - 1]
                    } else {
                        line[from] * (1.0 - next) + line[from + 1] * next
                    }
                })
                .collect()
        })
        .collect();
    let mut out = Vec::with_capacity(to * to);
    for &(from, next) in &taps {
        let (upper, lower) = (&widened[from], &widened[(from + 1).min(side - 1)]);
        out.extend(
            upper
                .iter()
                .zip(lower)
                .map(|(u, l)| u * (1.0 - next) + l * next),
        );
    }
    out
}

pub fn square(source: &[f32], side: usize, to: usize) -> Vec<f32> {
    match side.cmp(&to) {
        std::cmp::Ordering::Equal => source.to_vec(),
        std::cmp::Ordering::Greater => by_area(source, (side, side), (to, to), 1),
        std::cmp::Ordering::Less => enlarged(source, side, to),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn pattern(len: usize) -> Vec<u8> {
        (0..len as u64)
            .map(|i| ((i * 2_654_435_761) >> 7) as u8)
            .collect()
    }

    fn reference() -> Value {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/area.json"))
            .unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn size(case: &Value, key: &str) -> usize {
        case[key].as_u64().unwrap() as usize
    }

    #[test]
    fn a_picture_shrinks_by_area() {
        for case in reference()["scales"].as_array().unwrap() {
            let (w, h) = (size(case, "width"), size(case, "height"));
            let to = (size(case, "to_width"), size(case, "to_height"));
            let expected: Vec<u8> = case["pixels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let got = shrink(&pattern(w * h * 3), (w, h), to);
            let worst = got
                .iter()
                .zip(&expected)
                .map(|(g, e)| g.abs_diff(*e))
                .max()
                .unwrap();
            assert!(worst <= 1, "{w} x {h} to {to:?}: off by {worst}");
        }
    }

    #[test]
    fn a_cell_comes_to_32_by_32() {
        for case in reference()["cells"].as_array().unwrap() {
            let side = size(case, "side");
            let src: Vec<f32> = pattern(side * side)
                .into_iter()
                .map(|v| f32::from(v & 1))
                .collect();
            let got = square(&src, side, 32);
            for (i, (g, e)) in got
                .iter()
                .zip(case["values"].as_array().unwrap())
                .enumerate()
            {
                let e = e.as_f64().unwrap() as f32;
                assert!(
                    (g - e).abs() < 1e-5,
                    "side {side}, value {i}: {g} against {e}"
                );
            }
        }
    }
}
