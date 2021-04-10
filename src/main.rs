use clap::Arg;
use git2::Repository;
use std::collections::HashSet;

#[macro_use]
extern crate log;

mod detector;

type GitResult<T> = Result<T, git2::Error>;

pub struct Options {
  pub ignore_fixups: bool
}

fn _main(commit_ids: Vec<&str>, options: &Options) -> GitResult<()> {
  let repo = Repository::open(&std::env::current_dir().unwrap())?;
  let mut seen: HashSet<_> = HashSet::default();

  for id in commit_ids {
    let rev = repo.revparse_single(id)?;

    debug!("Finding deps for {}", rev.id());

    let commit = repo.find_commit(rev.id())?;
    let deps = detector::commit_dependencies(&repo, &commit, &mut seen, options)?;

    for dep in deps {
      seen.insert(dep);
    }
  }

  for dep in seen {
    println!("{}", dep);
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
        .multiple(true)
        .help("Commits for which to look up the dependencies for"),
    )
    .arg(
      Arg::with_name("debug")
        .short("d")
        .long("debug")
        .takes_value(false)
        .help("Enable debug logging"),
    )
    .arg(
      Arg::with_name("ignore-fixups")
        .long("ignore-fixups")
        .help("Ignore fixup commits (those prefixed with 'fixup!')")
    )
    .get_matches();

  if matches.is_present("debug") {
    simple_logging::log_to_stderr(log::LevelFilter::Debug);
  }

  let commits = matches.values_of("commit").unwrap().into_iter().collect();

  let options = Options {
    ignore_fixups: matches.is_present("ignore-fixups")
  };

  let res = _main(commits, &options);

  if let Err(e) = res {
    eprintln!("git error: {}", e);
    std::process::exit(1);
  }
}
