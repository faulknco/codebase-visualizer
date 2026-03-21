use crate::scene::{FileGraph, FileId, FileInfo};
use git2::{Repository, Sort};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub fn collect_file_graph(repo_path: &Path, max_commits: usize) -> FileGraph {
    let repo = Repository::open(repo_path).expect("Failed to open repo");

    let mut revwalk = repo.revwalk().expect("Failed to create revwalk");
    revwalk.push_head().expect("Failed to push HEAD");
    revwalk.set_sorting(Sort::TIME).expect("Failed to set sorting");

    let mut commit_files: Vec<HashSet<FileId>> = Vec::new();
    let mut file_commits: HashMap<FileId, Vec<i64>> = HashMap::new();
    let mut file_lines: HashMap<FileId, usize> = HashMap::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= max_commits {
            break;
        }
        let oid = match oid {
            Ok(o) => o,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .ok();

        let mut files_in_commit = HashSet::new();
        if let Some(diff) = diff {
            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path() {
                        let id = path.to_string_lossy().to_string();
                        files_in_commit.insert(id);
                    }
                    true
                },
                None,
                None,
                None,
            )
            .ok();
        }

        let timestamp = commit.time().seconds();
        for f in &files_in_commit {
            file_commits.entry(f.clone()).or_default().push(timestamp);
        }
        commit_files.push(files_in_commit);
    }

    for file_id in file_commits.keys() {
        let full_path = repo_path.join(file_id);
        if full_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                file_lines.insert(file_id.clone(), content.lines().count());
            }
        }
    }

    let mut files: HashMap<FileId, FileInfo> = HashMap::new();
    for (id, timestamps) in &file_commits {
        let full_path = repo_path.join(id);
        if !full_path.exists() {
            continue;
        }
        files.insert(
            id.clone(),
            FileInfo {
                path: PathBuf::from(id),
                lines: *file_lines.get(id).unwrap_or(&0),
                last_modified: *timestamps.iter().max().unwrap_or(&0),
                commit_count: timestamps.len(),
            },
        );
    }

    let file_ids: Vec<FileId> = files.keys().cloned().collect();
    let mut co_change = Vec::new();
    for i in 0..file_ids.len() {
        for j in (i + 1)..file_ids.len() {
            let a = &file_ids[i];
            let b = &file_ids[j];
            let together = commit_files.iter().filter(|c| c.contains(a) && c.contains(b)).count();
            if together == 0 { continue; }
            let either = commit_files.iter().filter(|c| c.contains(a) || c.contains(b)).count();
            let score = together as f32 / either as f32;
            co_change.push((a.clone(), b.clone(), score));
        }
    }

    FileGraph { files, co_change }
}

pub async fn run(repo_path: PathBuf, tx: mpsc::Sender<FileGraph>) {
    log::info!("GitCollector started for {}", repo_path.display());

    let graph = collect_file_graph(&repo_path, 500);
    log::info!("GitCollector: {} files, {} co-change pairs", graph.files.len(), graph.co_change.len());
    if tx.send(graph).await.is_err() { return; }

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let graph = collect_file_graph(&repo_path, 500);
        if tx.send(graph).await.is_err() { break; }
    }
}
