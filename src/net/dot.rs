pub fn product(a: &[f32], b: &[f32]) -> f32 {
    let (a_eights, a_rest) = a.as_chunks::<8>();
    let (b_eights, b_rest) = b.as_chunks::<8>();
    let mut sums = [0.0f32; 8];
    let mut i = 0;
    while i < a_eights.len() && i < b_eights.len() {
        let (x, y) = (&a_eights[i], &b_eights[i]);
        sums[0] += x[0] * y[0];
        sums[1] += x[1] * y[1];
        sums[2] += x[2] * y[2];
        sums[3] += x[3] * y[3];
        sums[4] += x[4] * y[4];
        sums[5] += x[5] * y[5];
        sums[6] += x[6] * y[6];
        sums[7] += x[7] * y[7];
        i += 1;
    }
    let mut rest = 0.0;
    let mut i = 0;
    while i < a_rest.len() && i < b_rest.len() {
        rest += a_rest[i] * b_rest[i];
        i += 1;
    }
    sums[0] + sums[1] + sums[2] + sums[3] + sums[4] + sums[5] + sums[6] + sums[7] + rest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_is_the_sum_of_the_pairs_multiplied() {
        let a: Vec<f32> = (0..19).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..19).map(|i| (i % 3) as f32).collect();
        let plain: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert_eq!(product(&a, &b), plain);
        assert_eq!(product(&a[..8], &b[..8]), 26.0);
        assert_eq!(product(&[], &[]), 0.0);
    }
}
