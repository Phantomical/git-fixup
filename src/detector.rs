use git2::{Commit, DiffDelta, DiffHunk, Oid, Repository};
use std::collections::{HashMap, HashSet};
use crate::GitResult;

pub fn commit_dependencies<'repo>(
  repo: &'repo Repository,
  commit: &Commit<'repo>,
) -> GitResult<Vec<Oid>> {
  let mut deps = vec![];

  for parent in commit.parents() {
    deps.append(&mut find_dependencies_with_parent(repo, &commit, &parent)?);
  }

  Ok(deps)
}

fn find_dependencies_with_parent<'repo>(
  repo: &'repo Repository,
  dependent: &Commit<'repo>,
  parent: &Commit<'repo>,
) -> GitResult<Vec<Oid>> {
  let mut options = git2::DiffOptions::new();
  options.include_unmodified(true);
  options.indent_heuristic(true);
  options.context_lines(1);
  let mut diff = repo.diff_tree_to_tree(
    Some(&parent.tree()?),
    Some(&dependent.tree()?),
    Some(&mut options),
  )?;

  let mut options = git2::DiffFindOptions::new();
  options.all(true);
  diff.find_similar(Some(&mut options))?;

  let mut blames = HashMap::new();
  let mut deps = HashSet::new();

  let mut err = Ok(());
  let mut cb = |delta: DiffDelta, hunk: DiffHunk| {
    let mut inner = |delta: DiffDelta, hunk: DiffHunk| -> GitResult<()> {
      let old_file = delta.old_file();
      if old_file.id().is_zero() {
        return Ok(());
      }

      let path = old_file.path().unwrap();
      let blame = match blames.get(path) {
        Some(blame) => blame,
        None => {
          let mut options = git2::BlameOptions::new();
          options.track_copies_same_commit_copies(true);
          options.track_copies_same_commit_moves(true);
          options.track_copies_same_file(true);
          let blame = repo.blame_file(path, Some(&mut options))?;
          blames.entry(path.to_owned()).or_insert(blame)
        }
      };

      for line in hunk.new_start()..hunk.new_start() + hunk.new_lines() {
        let bhunk = match blame.get_line(line as _) {
          Some(hunk) => hunk,
          None => continue,
        };

        deps.insert(bhunk.final_commit_id());
      }

      Ok(())
    };

    match inner(delta, hunk) {
      Ok(()) => true,
      Err(e) => {
        err = Err(e);
        false
      }
    }
  };
  diff.foreach(
    &mut |_, _| true,
    None,
    Some(&mut cb),
    Some(&mut |_, _, _| true),
  )?;
  err?;

  Ok(deps.into_iter().collect())
}
