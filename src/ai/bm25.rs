use std::collections::HashMap;

/// BM25 Okapi lexical ranker para busca ultra-rápida na memória do agente
#[allow(dead_code)]
pub struct BM25Ranker {
    k1: f64,
    b: f64,
    doc_lengths: Vec<usize>,
    avg_doc_length: f64,
    term_doc_freqs: HashMap<String, usize>,
    total_docs: usize,
    doc_terms: Vec<HashMap<String, usize>>,
}

#[allow(dead_code)]
impl BM25Ranker {
    pub fn new(documents: &[String]) -> Self {
        let k1 = 1.5;
        let b = 0.75;
        let total_docs = documents.len();
        let mut doc_lengths = Vec::with_capacity(total_docs);
        let mut term_doc_freqs: HashMap<String, usize> = HashMap::new();
        let mut doc_terms: Vec<HashMap<String, usize>> = Vec::with_capacity(total_docs);
        let mut total_length = 0;

        for doc in documents {
            let tokens = Self::tokenize(doc);
            let len = tokens.len();
            doc_lengths.push(len);
            total_length += len;

            let mut freqs: HashMap<String, usize> = HashMap::new();
            for t in tokens {
                *freqs.entry(t).or_insert(0) += 1;
            }

            for t in freqs.keys() {
                *term_doc_freqs.entry(t.clone()).or_insert(0) += 1;
            }

            doc_terms.push(freqs);
        }

        let avg_doc_length = if total_docs > 0 {
            total_length as f64 / total_docs as f64
        } else {
            0.0
        };

        Self {
            k1,
            b,
            doc_lengths,
            avg_doc_length,
            term_doc_freqs,
            total_docs,
            doc_terms,
        }
    }

    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| w.to_string())
            .collect()
    }

    pub fn score(&self, query: &str) -> Vec<(usize, f64)> {
        let q_tokens = Self::tokenize(query);
        let mut scores = Vec::with_capacity(self.total_docs);

        for doc_idx in 0..self.total_docs {
            let doc_len = self.doc_lengths[doc_idx] as f64;
            let doc_term_map = &self.doc_terms[doc_idx];
            let mut score = 0.0;

            for q in &q_tokens {
                if let Some(&df) = self.term_doc_freqs.get(q) {
                    let idf = ((self.total_docs as f64 - df as f64 + 0.5) / (df as f64 + 0.5) + 1.0).ln();
                    let tf = *doc_term_map.get(q).unwrap_or(&0) as f64;
                    let numerator = tf * (self.k1 + 1.0);
                    let denominator = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_length.max(1.0)));
                    score += idf * (numerator / denominator);
                }
            }

            scores.push((doc_idx, score));
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bm25_ranking() {
        let docs = vec![
            "O bot do jogo Huntera caiu por erro de WebSocket.".to_string(),
            "O banco de dados SQLite registrou uma nova transação de gold.".to_string(),
            "O Cloudflare Turnstile travou a autenticação da sala.".to_string(),
        ];
        let ranker = BM25Ranker::new(&docs);
        let scores = ranker.score("Huntera WebSocket");
        assert_eq!(scores[0].0, 0, "O documento mais relevante deveria ser o índice 0");
    }
}
