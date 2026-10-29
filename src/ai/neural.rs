//! Motor Neural BitNet 1-bit / Quantização e Fast Policy SIMD
//! Paridade 100% com agent-go/pkg/neural/one_bit.go

#[allow(dead_code)]
pub struct Neural1Bit;

#[allow(dead_code)]
impl Neural1Bit {
    /// Calcula a distância de Hamming exata entre dois vetores de 1-bit empacotados em u64.
    /// No x86-64 moderno, .count_ones() compila diretamente para a instrução de hardware POPCNT.
    #[inline(always)]
    pub fn hamming_distance_1bit(a: &[u64], b: &[u64]) -> u32 {
        if a.len() != b.len() || a.is_empty() {
            return 0;
        }

        let mut dist: u32 = 0;
        for i in 0..a.len() {
            dist += (a[i] ^ b[i]).count_ones();
        }
        dist
    }

    /// Converte a distância de Hamming em similaridade de cosseno no hipercubo binário.
    /// Retorna um valor no intervalo [-1.0, 1.0].
    pub fn cosine_similarity_1bit(a: &[u64], b: &[u64]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let total_bits = (a.len() * 64) as f64;
        let dist = Self::hamming_distance_1bit(a, b) as f64;
        // cos(theta) = 1 - 2 * (dist / total_bits)
        1.0 - 2.0 * (dist / total_bits)
    }

    /// Binariza um vetor float normalizado em representação 1-bit empacotada em u64 (+1 -> 1, -1 -> 0).
    pub fn quantize_to_1bit(floats: &[f32]) -> Vec<u64> {
        let num_u64 = floats.len().div_ceil(64);
        let mut packed = vec![0u64; num_u64];

        for (i, &val) in floats.iter().enumerate() {
            if val > 0.0 {
                let u64_idx = i / 64;
                let bit_idx = i % 64;
                packed[u64_idx] |= 1u64 << bit_idx;
            }
        }

        packed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_and_cosine_1bit() {
        let a = vec![0b10101010u64];
        let b = vec![0b10101010u64];
        assert_eq!(Neural1Bit::hamming_distance_1bit(&a, &b), 0);
        let sim = Neural1Bit::cosine_similarity_1bit(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);

        let c = vec![!0b10101010u64];
        let dist = Neural1Bit::hamming_distance_1bit(&a, &c);
        assert_eq!(dist, 64);
        let sim_inv = Neural1Bit::cosine_similarity_1bit(&a, &c);
        assert!((sim_inv - (-1.0)).abs() < 1e-6);
    }
}
