use std::process::Command;
use tempfile::TempDir;

fn create_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();

    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(p)
            .output()
            .expect("git command failed");
    };

    run(&["init"]);
    run(&["-c", "user.name=Test", "-c", "user.email=test@test.com", "commit", "--allow-empty", "-m", "init"]);

    // Commit 1: a.rs + b.rs together
    std::fs::write(p.join("a.rs"), "fn a() {}").unwrap();
    std::fs::write(p.join("b.rs"), "fn b() {}").unwrap();
    run(&["add", "."]);
    run(&["-c", "user.name=Test", "-c", "user.email=test@test.com", "commit", "-m", "add a and b"]);

    // Commit 2: a.rs + c.rs together
    std::fs::write(p.join("a.rs"), "fn a() { updated }").unwrap();
    std::fs::write(p.join("c.rs"), "fn c() {}").unwrap();
    run(&["add", "."]);
    run(&["-c", "user.name=Test", "-c", "user.email=test@test.com", "commit", "-m", "update a, add c"]);

    dir
}

#[test]
fn test_co_change_matrix() {
    let dir = create_test_repo();
    let graph = cviz::pipeline::git_collector::collect_file_graph(dir.path(), 500);

    assert_eq!(graph.files.len(), 3);
    assert_eq!(graph.files["a.rs"].commit_count, 2);
    assert_eq!(graph.files["b.rs"].commit_count, 1);

    let ab_score = graph.co_change.iter()
        .find(|(a, b, _)| (a == "a.rs" && b == "b.rs") || (a == "b.rs" && b == "a.rs"))
        .map(|(_, _, s)| *s)
        .unwrap_or(0.0);
    assert!((ab_score - 0.5).abs() < 0.01, "a+b score was {}", ab_score);

    let ac_score = graph.co_change.iter()
        .find(|(a, b, _)| (a == "a.rs" && b == "c.rs") || (a == "c.rs" && b == "a.rs"))
        .map(|(_, _, s)| *s)
        .unwrap_or(0.0);
    assert!((ac_score - 0.5).abs() < 0.01, "a+c score was {}", ac_score);
}
