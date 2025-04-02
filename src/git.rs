use anyhow::{bail, Context, Result};
use git2::{Delta, Oid, Repository};
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

pub struct Treeish {
    commit1: String,
    commit2: Option<String>,
}
impl Treeish {
    pub fn new(commit1: String, commit2: Option<String>) -> Self {
        Treeish { commit1, commit2 }
    }
}

/// Get a list of changed files in the Git repository at `repo_path`.
pub fn get_changed_files(
    repo_path: PathBuf,
    trees_to_diff: Option<Treeish>,
) -> Result<Vec<ChangedFile>> {
    let repo = Repository::discover(&repo_path).context("Failed to open Git repository")?;
    let Some(repo_path) = repo.workdir() else {
        bail!("Couldn't get workdir from {}", repo_path.display());
    };

    let diff = match trees_to_diff {
        // emulates `git diff`
        None => repo
            .diff_index_to_workdir(None, None)
            .context("Couldn't get diff from index to workdir")?,
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
                let c1_tree = repo.find_tree(c1_oid)?;
                let c2_tree = c2_oid.map(|oid| repo.find_tree(oid));

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
                None => repo.diff_tree_to_workdir(Some(&c1_tree), None)?,
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

    let mut changed_files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            match delta.status() {
                Delta::Added => {
                    let Some(path) = delta.new_file().path() else {
                        // Ignore files without a path
                        return true;
                    };

                    let full_path = repo_path.join(path);

                    let contents = match fs::read_to_string(full_path) {
                        Ok(contents) => contents,
                        Err(e) => {
                            eprintln!("Couldn't read {}: {e}", path.display());
                            return true;
                        }
                    };

                    let change = ChangedFile {
                        path: path.to_path_buf(),
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

                    // old file is in the index, new file is in the workdir,
                    // according to Repository::diff_index_to_workdir docs
                    let before_contents = get_blob_contents(&repo, &old_oid).unwrap();
                    let new_full_path = repo_path.join(new_path);
                    let after_contents = match fs::read_to_string(&new_full_path) {
                        Ok(contents) => contents,
                        Err(e) => {
                            eprintln!("Couldn't read {}: {e}", new_full_path.display());
                            return true;
                        }
                    };

                    // TODO: is this true?
                    assert_eq!(new_path, old_path);
                    let change = ChangedFile {
                        // assumes the above assert holds
                        path: new_full_path,
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
                        path: path.to_path_buf(),
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
        let changed_files = get_changed_files(repo_path, None, None).unwrap();
        for file in changed_files {
            println!("{file}");
        }
    }
}
