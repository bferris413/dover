use anyhow::{Context, Result, bail};
use git2::{Delta, FileMode, Oid, Repository};
use std::{fmt::Display, fs, path::PathBuf};

/// Represents the type of change for a file.
#[derive(Debug)]
pub enum Change {
    Added {
        contents: String,
    },
    Modified {
        before_contents: String,
        after_contents: String,
    },
    Deleted {
        contents: String,
    },
}
impl Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Change::*;
        match self {
            Added { .. } => write!(f, "+"),
            Modified { .. } => write!(f, "~"),
            Deleted { .. } => write!(f, "-"),
        }
    }
}

/// Represents a changed file with its path and type of change.
#[derive(Debug)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub change_type: Change,
}
impl Display for ChangedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.change_type, self.path.display())
    }
}

#[derive(Clone, Debug)]
pub struct Treeish {
    commit1: String,
    commit2: Option<String>,
}
impl Treeish {
    pub fn new(commit1: String, commit2: Option<String>) -> Self {
        Treeish { commit1, commit2 }
    }
}

#[derive(Debug)]
pub struct RepoChangedFiles {
    pub changed_files: Vec<ChangedFile>,
    pub repo_workdir: PathBuf,
}

/// Get a list of changed files in the Git repository at `repo_path`.
pub fn get_changed_files(
    repo_path: PathBuf,
    trees_to_diff: Option<Treeish>,
) -> Result<RepoChangedFiles> {
    let repo = Repository::discover(&repo_path).context("Failed to open Git repository")?;
    let Some(repo_path) = repo.workdir() else {
        bail!("Couldn't get workdir from {}", repo_path.display());
    };

    let mut index_to_workdir = false;
    let diff = match trees_to_diff {
        // emulates `git diff`
        None => {
            index_to_workdir = true;
            repo.diff_index_to_workdir(None, None)
                .context("Couldn't get diff from index to workdir")?
        }
        Some(Treeish { commit1, commit2 }) => {
            let (c1_tree, c2_tree) = {
                // we need to find the tree associated with each commit
                let c1_oid = Oid::from_str(&commit1).context("Couldn't parse commit1 OID")?;
                let c2_oid =
                    commit2.map(|oid| Oid::from_str(&oid).context("Couldn't parse commit2 OID"));

                let c2_oid = match c2_oid {
                    Some(Err(e)) => bail!("Couldn't parse commit2 OID: {}", e),
                    Some(Ok(oid)) => Some(oid),
                    None => None,
                };

                // let c1_tree = dbg!(repo.find_tree(c1_oid))?;
                let c1_tree = repo.find_commit(c1_oid).and_then(|c1| c1.tree())?;
                let c2_tree = c2_oid.map(|oid| repo.find_commit(oid).and_then(|c| c.tree()));
                // let c2_tree = c2_oid.map(|oid| repo.find_tree(oid));

                if let Some(Err(e)) = c2_tree {
                    bail!("Couldn't find tree for commit2: {}", e);
                }
                let c2_tree = c2_tree.map(|tree| tree.unwrap());

                (c1_tree, c2_tree)
            };
            match c2_tree {
                Some(_) => repo
                    .diff_tree_to_tree(Some(&c1_tree), c2_tree.as_ref(), None)
                    .context("Couldn't get diff from {commit1}")?,
                None => repo.diff_tree_to_workdir_with_index(Some(&c1_tree), None)?,
            }
        }
    };

    // let head = repo.head().context("Failed to get HEAD")?;
    // let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;
    // let tree = head_commit
    //     .tree()
    //     .context("Failed to get tree from HEAD commit")?;
    // let index = repo.index().context("Failed to get index")?;

    // let diff = repo
    //     .diff_tree_to_index(Some(&tree), Some(&index), None)
    //     .context("Failed to get diff from tree to index")?;

    // This is fundamentally broken since it doesn't account for differences between
    // workdir changes and stuff in the index or an old commit
    let mut changed_files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            // Check if the entry is a regular file
            let is_blob = |mode| matches!(mode, FileMode::Blob | FileMode::BlobExecutable);
            let is_regular_file = match delta.status() {
                Delta::Added => is_blob(delta.new_file().mode()),
                Delta::Modified => {
                    is_blob(delta.old_file().mode()) && is_blob(delta.new_file().mode())
                }
                Delta::Deleted => is_blob(delta.old_file().mode()),
                _ => false, // Ignore other types of changes
            };

            if !is_regular_file {
                // Skip submodules or non-regular files
                return true;
            }

            match delta.status() {
                Delta::Added => {
                    let Some(path) = delta.new_file().path() else {
                        // Ignore files without a path
                        return true;
                    };
                    let new_oid = delta.new_file().id();

                    let contents = if index_to_workdir {
                        // new file is in the workdir,
                        // according to Repository::diff_index_to_workdir docs
                        let new_full_path = repo_path.join(path);
                        match fs::read_to_string(&new_full_path) {
                            Ok(contents) => contents,
                            Err(e) => {
                                eprintln!("Couldn't read {}: {e}", new_full_path.display());
                                return true;
                            }
                        }
                    } else {
                        // new file is in the repo
                        get_blob_contents(&repo, &new_oid).unwrap()
                    };

                    let change = ChangedFile {
                        path: {
                            let relative_path = repo_path.file_name().unwrap();
                            let mut rel_path = PathBuf::from(relative_path);
                            rel_path.push(path);
                            rel_path
                        },
                        change_type: Change::Added { contents },
                    };
                    changed_files.push(change);
                }
                Delta::Modified => {
                    let (Some(new_path), Some(old_path)) =
                        (delta.new_file().path(), delta.old_file().path())
                    else {
                        // Ignore files without a path
                        return true;
                    };

                    let old_oid = delta.old_file().id();
                    let new_oid = delta.new_file().id();
                    if new_oid.is_zero() || old_oid.is_zero() {
                        // Ignore files without a valid OID
                        return true;
                    }

                    let before_contents = get_blob_contents(&repo, &old_oid).unwrap();
                    let after_contents = if index_to_workdir {
                        // old file is in the index, new file is in the workdir,
                        // according to Repository::diff_index_to_workdir docs
                        //
                        let new_full_path = repo_path.join(new_path);
                        match fs::read_to_string(&new_full_path) {
                            Ok(contents) => contents,
                            Err(e) => {
                                eprintln!("Couldn't read {}: {e}", new_full_path.display());
                                return true;
                            }
                        }
                    } else {
                        // new file is in the repo
                        get_blob_contents(&repo, &new_oid).unwrap()
                    };

                    // TODO: is this true?
                    assert_eq!(new_path, old_path);
                    let change = ChangedFile {
                        // assumes the above assert holds
                        path: {
                            let relative_path = repo_path.file_name().unwrap();
                            let mut rel_path = PathBuf::from(relative_path);
                            rel_path.push(new_path);
                            rel_path
                        },
                        change_type: Change::Modified {
                            before_contents,
                            after_contents,
                        },
                    };
                    changed_files.push(change);
                }
                Delta::Deleted => {
                    let Some(path) = delta.old_file().path() else {
                        // Ignore files without a path
                        return true;
                    };

                    let oid = delta.old_file().id();
                    if oid.is_zero() {
                        // Ignore files without a valid OID
                        return true;
                    }
                    let contents = get_blob_contents(&repo, &oid).unwrap();

                    let change = ChangedFile {
                        path: {
                            let relative_path = repo_path.file_name().unwrap();
                            let mut rel_path = PathBuf::from(relative_path);
                            rel_path.push(path);
                            rel_path
                        },
                        change_type: Change::Deleted { contents },
                    };
                    changed_files.push(change);
                }
                _ => return true, // Ignore other types of changes
            };

            true
        },
        None,
        None,
        None,
    )
    .context("Failed to iterate over diff")?;

    let repo_files = RepoChangedFiles {
        changed_files,
        repo_workdir: repo_path.to_path_buf(),
    };
    Ok(repo_files)
}

/// Get the contents of a blob by its OID.
fn get_blob_contents(repo: &Repository, oid: &Oid) -> Result<String> {
    let blob = repo.find_blob(*oid).context("Failed to find blob")?;
    let contents =
        std::str::from_utf8(blob.content()).context("Failed to convert blob contents to string")?;
    Ok(contents.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("dover-git-test-{unique}"));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }

    fn commit_all(repo: &Repository, message: &str) -> Oid {
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();

        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let signature = Signature::now("Dover Tests", "dover@example.com").unwrap();
        let parents = repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| repo.find_commit(oid).unwrap());
        let parent_refs = parents.iter().collect::<Vec<_>>();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .unwrap()
    }

    #[test]
    fn includes_added_and_deleted_files() {
        let temp_repo = TempRepo::new();
        let repo = Repository::init(&temp_repo.path).unwrap();

        let deleted_contents = "pub struct Deleted;\n";
        fs::write(temp_repo.path.join("deleted.rs"), deleted_contents).unwrap();
        let first_commit = commit_all(&repo, "add deleted.rs");

        fs::remove_file(temp_repo.path.join("deleted.rs")).unwrap();
        let added_contents = "pub struct Added;\n";
        fs::write(temp_repo.path.join("added.rs"), added_contents).unwrap();
        let second_commit = commit_all(&repo, "replace deleted.rs with added.rs");

        let changes = get_changed_files(
            temp_repo.path.clone(),
            Some(Treeish::new(
                first_commit.to_string(),
                Some(second_commit.to_string()),
            )),
        )
        .unwrap();

        assert_eq!(changes.changed_files.len(), 2);
        assert!(changes.changed_files.iter().any(|file| {
            file.path.ends_with("added.rs")
                && matches!(
                    &file.change_type,
                    Change::Added { contents } if contents == added_contents
                )
        }));
        assert!(changes.changed_files.iter().any(|file| {
            file.path.ends_with("deleted.rs")
                && matches!(
                    &file.change_type,
                    Change::Deleted { contents } if contents == deleted_contents
                )
        }));
    }
}
