use anyhow::{Context, Result};
use git2::{Delta, Oid, Repository};
use std::{fmt::Display, fs, path::PathBuf};

/// Represents the type of change for a file.
#[derive(Debug)]
pub enum ChangeType {
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
impl Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ChangeType::*;
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
    pub change_type: ChangeType,
}
impl Display for ChangedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.change_type, self.path.display())
    }
}

/// Get a list of changed files in the Git repository at `repo_path`.
pub fn get_changed_files(repo_path: PathBuf) -> Result<Vec<ChangedFile>> {
    let repo = Repository::open(repo_path).context("Failed to open Git repository")?;
    let diff = repo
        .diff_index_to_workdir(None, None)
        .context("Failed to get diff from index to workdir")?;

    // let head = repo.head().context("Failed to get HEAD")?;
    // let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;
    // let tree = head_commit
    //     .tree()
    //     .context("Failed to get tree from HEAD commit")?;
    // let index = repo.index().context("Failed to get index")?;

    // let diff = repo
    //     .diff_tree_to_index(Some(&tree), Some(&index), None)
    //     .context("Failed to get diff from tree to index")?;

    let mut changed_files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            match delta.status() {
                Delta::Added => {
                    let Some(path) = delta.new_file().path() else {
                        // Ignore files without a path
                        return true;
                    };

                    let contents = match fs::read_to_string(path) {
                        Ok(contents) => contents,
                        Err(e) => {
                            eprintln!("Couldn't read {}: {e}", path.display());
                            return true;
                        }
                    };

                    let change = ChangedFile {
                        path: path.to_path_buf(),
                        change_type: ChangeType::Added { contents },
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

                    // old file is in the index, new file is in the workdir,
                    // according to Repository::diff_index_to_workdir docs
                    let before_contents = get_blob_contents(&repo, &old_oid).unwrap();
                    let after_contents = match fs::read_to_string(new_path) {
                        Ok(contents) => contents,
                        Err(e) => {
                            eprintln!("Couldn't read {}: {e}", new_path.display());
                            return true;
                        }
                    };

                    // TODO: is this true?
                    assert_eq!(new_path, old_path);
                    let change = ChangedFile {
                        // assumes the above assert holds
                        path: new_path.to_path_buf(),
                        change_type: ChangeType::Modified {
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
                        path: path.to_path_buf(),
                        change_type: ChangeType::Deleted { contents },
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

    Ok(changed_files)
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

    #[test]
    fn test_get_changed_files() {
        let repo_path = PathBuf::from(".");
        let changed_files = get_changed_files(repo_path).unwrap();
        for file in changed_files {
            println!("{file}");
        }
    }
}
