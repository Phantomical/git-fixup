use crate::{GitResult, Options};
use git2::{Commit, DiffDelta, DiffHunk, Oid, Repository};
use std::collections::{HashMap, HashSet};

pub fn commit_dependencies<'repo>(
  repo: &'repo Repository,
  commit: &Commit<'repo>,
  seen: &mut HashSet<Oid>,
  options: &Options,
) -> GitResult<Vec<Oid>> {
  let mut deps = vec![];
  let mut stack: Vec<_> = commit.parents().collect();

  while let Some(parent) = stack.pop() {
    let found = find_dependencies_with_parent(repo, &commit, &parent)?;

    for oid in found {
      if seen.contains(&oid) {
        continue;
      }

      seen.insert(oid);
      let commit = repo.find_commit(oid)?;

      if options.ignore_fixups && commit.message_bytes().starts_with(b"fixup! ") {
        stack.push(commit);
      } else {
        deps.push(oid);
      }
    }
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
          options.newest_commit(parent.id());
          let blame = repo.blame_file(path, Some(&mut options))?;
          blames.entry(path.to_owned()).or_insert(blame)
        }
      };

      for line in hunk.old_start()..hunk.old_start() + hunk.old_lines() {
        let bhunk = match blame.get_line(line as _) {
          Some(hunk) => hunk,
          None => continue,
        };

        debug!("Scanning line {} of {}", line, path.display());
        debug!("Found dep: {}", bhunk.final_commit_id());

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
