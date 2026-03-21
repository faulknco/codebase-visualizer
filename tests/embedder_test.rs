#[test]
fn test_tfidf_similar_files_closer() {
    use std::collections::HashMap;

    let files: HashMap<String, String> = HashMap::from([
        ("a.rs".into(), "fn main() { let x = compute(); println!(x); }".into()),
        ("b.rs".into(), "fn helper() { let y = compute(); return y; }".into()),
        ("readme.md".into(), "# Project\nThis is a readme with no code".into()),
    ]);

    let embeddings = cviz::pipeline::embedder::compute_tfidf(&files);

    assert_eq!(embeddings.len(), 3);

    let sim_ab = cosine_sim(&embeddings["a.rs"], &embeddings["b.rs"]);
    let sim_ac = cosine_sim(&embeddings["a.rs"], &embeddings["readme.md"]);
    assert!(sim_ab > sim_ac, "a-b sim ({}) should exceed a-readme sim ({})", sim_ab, sim_ac);
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}
