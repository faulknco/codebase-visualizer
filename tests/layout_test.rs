use std::collections::HashMap;
use cviz::scene::*;
use cviz::pipeline::layout::compute_layout;

#[test]
fn test_connected_files_closer_than_unconnected() {
    let mut files = HashMap::new();
    files.insert("a.rs".into(), FileInfo { path: "a.rs".into(), lines: 100, last_modified: 1000, commit_count: 10 });
    files.insert("b.rs".into(), FileInfo { path: "b.rs".into(), lines: 50, last_modified: 900, commit_count: 5 });
    files.insert("z.rs".into(), FileInfo { path: "z.rs".into(), lines: 30, last_modified: 100, commit_count: 1 });

    let graph = FileGraph {
        files,
        co_change: vec![("a.rs".into(), "b.rs".into(), 0.8)],
    };

    let embeddings = HashMap::from([
        ("a.rs".into(), vec![1.0, 0.0]),
        ("b.rs".into(), vec![0.9, 0.1]),
        ("z.rs".into(), vec![0.0, 1.0]),
    ]);
    let embed_map = EmbeddingMap { embeddings };

    let scene = compute_layout(&graph, &embed_map, DepthMode::Recency, ColorMode::FileType, 200);

    let a = scene.nodes.iter().find(|n| n.id == "a.rs").unwrap();
    let b = scene.nodes.iter().find(|n| n.id == "b.rs").unwrap();
    let z = scene.nodes.iter().find(|n| n.id == "z.rs").unwrap();

    let dist_ab = a.pos.distance(b.pos);
    let dist_az = a.pos.distance(z.pos);

    assert!(dist_ab < dist_az, "a-b ({:.2}) should be closer than a-z ({:.2})", dist_ab, dist_az);
}
