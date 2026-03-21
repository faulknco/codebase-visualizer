use crate::scene::{EmbeddingMap, FileGraph};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in s.split(|c: char| c.is_whitespace() || "{}()[];,:.\"'`#=<>+-*/&|!?@".contains(c)) {
        if word.is_empty() { continue; }
        let mut start = 0;
        let chars: Vec<char> = word.chars().collect();
        for i in 1..chars.len() {
            if chars[i].is_uppercase() && chars[i - 1].is_lowercase() {
                let token = chars[start..i].iter().collect::<String>().to_lowercase();
                if token.len() > 1 { tokens.push(token); }
                start = i;
            }
        }
        let remainder: String = chars[start..].iter().collect();
        for part in remainder.split('_') {
            let t = part.to_lowercase();
            if t.len() > 1 { tokens.push(t); }
        }
    }
    tokens
}

pub fn compute_tfidf(files: &HashMap<String, String>) -> HashMap<String, Vec<f32>> {
    let n_docs = files.len() as f32;

    let file_tokens: HashMap<&str, Vec<String>> = files
        .iter()
        .map(|(id, content)| (id.as_str(), tokenize(content)))
        .collect();

    let mut vocab: HashSet<String> = HashSet::new();
    let mut doc_freq: HashMap<String, usize> = HashMap::new();

    for tokens in file_tokens.values() {
        let unique: HashSet<&String> = tokens.iter().collect();
        for t in unique {
            *doc_freq.entry(t.clone()).or_default() += 1;
            vocab.insert(t.clone());
        }
    }

    let mut vocab: Vec<String> = vocab.into_iter().collect();
    vocab.sort();

    let token_to_idx: HashMap<&str, usize> = vocab.iter().enumerate().map(|(i, t)| (t.as_str(), i)).collect();
    let dim = vocab.len();

    let mut embeddings = HashMap::new();
    for (id, tokens) in &file_tokens {
        let mut tf: HashMap<&str, f32> = HashMap::new();
        for t in tokens {
            *tf.entry(t.as_str()).or_default() += 1.0;
        }
        let max_tf = tf.values().cloned().fold(0.0f32, f32::max).max(1.0);

        let mut vec = vec![0.0f32; dim];
        for (token, count) in &tf {
            if let Some(&idx) = token_to_idx.get(token) {
                let tf_norm = count / max_tf;
                let idf = (n_docs / *doc_freq.get(*token).unwrap_or(&1) as f32).ln() + 1.0;
                vec[idx] = tf_norm * idf;
            }
        }

        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec { *v /= norm; }
        }

        embeddings.insert(id.to_string(), vec);
    }

    embeddings
}

pub async fn run(
    repo_path: PathBuf,
    mut graph_rx: mpsc::Receiver<FileGraph>,
    tx: mpsc::Sender<(FileGraph, EmbeddingMap)>,
    shared_graph: Arc<RwLock<Option<FileGraph>>>,
) {
    eprintln!("[cviz] Embedder started");

    while let Some(graph) = graph_rx.recv().await {
        let mut file_contents: HashMap<String, String> = HashMap::new();
        for (id, info) in &graph.files {
            let full_path = repo_path.join(&info.path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                file_contents.insert(id.clone(), content);
            }
        }

        let raw_embeddings = compute_tfidf(&file_contents);
        let embed_map = EmbeddingMap { embeddings: raw_embeddings };

        eprintln!("[cviz] Embedder: computed {} embeddings", embed_map.embeddings.len());

        if tx.send((graph.clone(), embed_map)).await.is_err() { break; }
        *shared_graph.write().await = Some(graph);
    }
}
