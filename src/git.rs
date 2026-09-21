use anyhow::{Context, Result, bail};
use git2::{Delta, FileMode, Oid, Repository, Tree};
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
    revision1: String,
    revision2: Option<String>,
}
impl Treeish {
    pub fn new(revision1: String, revision2: Option<String>) -> Self {
        Treeish {
            revision1,
            revision2,
        }
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

    let mut new_side_is_workdir = false;
    let diff = match trees_to_diff {
        // emulates `git diff`
        None => {
            new_side_is_workdir = true;
            repo.diff_index_to_workdir(None, None)
                .context("Couldn't get diff from index to workdir")?
        }
        Some(Treeish {
            revision1,
            revision2,
        }) => {
            let revision1_tree = resolve_tree(&repo, &revision1)?;
            let revision2_tree = revision2
                .as_deref()
                .map(|revision| resolve_tree(&repo, revision))
                .transpose()?;

            match revision2_tree {
                Some(_) => repo
                    .diff_tree_to_tree(Some(&revision1_tree), revision2_tree.as_ref(), None)
                    .with_context(|| {
                        format!(
                            "Couldn't diff revisions `{revision1}` and `{}`",
                            revision2.as_deref().unwrap()
                        )
                    })?,
                None => {
                    new_side_is_workdir = true;
                    repo.diff_tree_to_workdir_with_index(Some(&revision1_tree), None)
                        .with_context(|| {
                            format!("Couldn't diff revision `{revision1}` against the worktree")
                        })?
                }
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

                    let contents = if new_side_is_workdir {
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
                    let after_contents = if new_side_is_workdir {
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

/// Resolve any Git revision expression that names a tree-ish object.
fn resolve_tree<'repo>(repo: &'repo Repository, revision: &str) -> Result<Tree<'repo>> {
    repo.revparse_single(revision)
        .with_context(|| format!("Couldn't resolve Git revision `{revision}`"))?
        .peel_to_tree()
        .with_context(|| format!("Git revision `{revision}` does not resolve to a tree"))
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_REPO: AtomicU64 = AtomicU64::new(0);

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let unique = NEXT_TEMP_REPO.fetch_add(1, Ordering::Relaxed);
            let process = std::process::id();
            let path = std::env::temp_dir().join(format!("dover-git-test-{process}-{unique}"));
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

    fn create_branch(repo: &Repository, name: &str, target: Oid) {
        let commit = repo.find_commit(target).unwrap();
        repo.branch(name, &commit, false).unwrap();
    }

    fn assert_modified_file(changes: &RepoChangedFiles, path: &str, before: &str, after: &str) {
        assert_eq!(changes.changed_files.len(), 1);
        let changed_file = &changes.changed_files[0];
        assert!(changed_file.path.ends_with(path));
        assert!(matches!(
            &changed_file.change_type,
            Change::Modified {
                before_contents,
                after_contents,
            } if before_contents == before && after_contents == after
        ));
    }

    #[test]
    fn diffs_two_branch_names() {
        let temp_repo = TempRepo::new();
        let repo = Repository::init(&temp_repo.path).unwrap();
        let file_path = temp_repo.path.join("example.rs");

        let before = "pub struct Example;\n";
        fs::write(&file_path, before).unwrap();
        let before_commit = commit_all(&repo, "before");
        create_branch(&repo, "demo/before", before_commit);

        let after = "pub struct Example { pub value: u8 }\n";
        fs::write(&file_path, after).unwrap();
        let after_commit = commit_all(&repo, "after");
        create_branch(&repo, "demo/after", after_commit);

        let changes = get_changed_files(
            temp_repo.path.clone(),
            Some(Treeish::new(
                "demo/before".to_owned(),
                Some("demo/after".to_owned()),
            )),
        )
        .unwrap();

        assert_modified_file(&changes, "example.rs", before, after);
    }

    #[test]
    fn diffs_branch_name_against_worktree() {
        let temp_repo = TempRepo::new();
        let repo = Repository::init(&temp_repo.path).unwrap();
        let file_path = temp_repo.path.join("example.rs");

        let before = "pub fn value() -> u8 { 1 }\n";
        fs::write(&file_path, before).unwrap();
        let base_commit = commit_all(&repo, "base");
        create_branch(&repo, "demo-base", base_commit);

        let after = "pub fn value() -> u8 { 2 }\n";
        fs::write(&file_path, after).unwrap();

        let changes = get_changed_files(
            temp_repo.path.clone(),
            Some(Treeish::new("demo-base".to_owned(), None)),
        )
        .unwrap();

        assert_modified_file(&changes, "example.rs", before, after);
    }

    #[test]
    fn diffs_relative_revision_expressions() {
        let temp_repo = TempRepo::new();
        let repo = Repository::init(&temp_repo.path).unwrap();
        let file_path = temp_repo.path.join("example.rs");

        let before = "pub const VALUE: u8 = 1;\n";
        fs::write(&file_path, before).unwrap();
        commit_all(&repo, "before");

        let after = "pub const VALUE: u8 = 2;\n";
        fs::write(&file_path, after).unwrap();
        commit_all(&repo, "after");

        let changes = get_changed_files(
            temp_repo.path.clone(),
            Some(Treeish::new("HEAD~1".to_owned(), Some("HEAD".to_owned()))),
        )
        .unwrap();

        assert_modified_file(&changes, "example.rs", before, after);
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
