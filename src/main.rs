use clap::Arg;
use git2::Repository;

mod detector;

type GitResult<T> = Result<T, git2::Error>;

fn _main(commit_ids: Vec<&str>) -> GitResult<()> {
  let repo = Repository::open(&std::env::current_dir().unwrap())?;

  for id in commit_ids {
    let rev = repo.revparse_single(id)?;
    let commit = repo.find_commit(rev.id())?;
    let deps = detector::commit_dependencies(&repo, &commit)?;

    for dep in deps {
      println!("{}", dep);
    }
  }

  Ok(())
}

fn main() {
  let matches = clap::App::new("git-deps")
    .about("Auto-detects commits on which the given commit(s) depend")
    .usage("git deps [options] COMMIT-ISH [COMMIT-ISH]...")
    .arg(
      Arg::with_name("commit")
        .required(true)
        .index(1)
        .multiple(true),
    )
    .get_matches();

  let res = _main(matches.values_of("commit").unwrap().into_iter().collect());

  if let Err(e) = res {
    eprintln!("git error: {}", e);
    std::process::exit(1);
  }

  if true {}
}
