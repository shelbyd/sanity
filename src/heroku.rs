use regex::Regex;
use serde::*;
use std::path::{Path, PathBuf};

use crate::utils::run;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    buildpacks: Vec<String>,

    app: String,

    #[serde(default)]
    copy_to_root: Vec<PathBuf>,
}

pub fn deploy(path: &Path, config: Config) -> anyhow::Result<()> {
    match_buildpacks(&config)?;
    copy_files_to_root(path, &config.copy_to_root)?;
    let deploy_tag = create_deploy_branch(&config)?;
    run(format!(
        "git push https://git.heroku.com/{app}.git {tag}:master --force",
        app = config.app,
        tag = deploy_tag
    ))?;
    run(format!("git branch -D {}", deploy_tag))?;
    Ok(())
}

fn match_buildpacks(config: &Config) -> anyhow::Result<()> {
    while let Some(action) =
        next_match_action(&dbg!(current_buildpacks(config)?), &config.buildpacks)
    {
        match action {
            Action::Remove(i) => {
                run(format!(
                    "heroku buildpacks:remove --app {} --index {}",
                    config.app,
                    i + 1
                ))?;
            }
            Action::Insert(i, v) => {
                run(format!(
                    "heroku buildpacks:add --app {} --index {} {}",
                    config.app,
                    i + 1,
                    v
                ))?;
            }
        }
    }
    Ok(())
}

fn current_buildpacks(config: &Config) -> anyhow::Result<Vec<String>> {
    let output = run(format!("heroku buildpacks --app {}", config.app))?;

    let mut result = Vec::new();

    let many_packs = Regex::new(r"(?m)^(\d+)\. (.+)$").unwrap();
    for cap in many_packs.captures_iter(&output) {
        result.insert(cap[1].parse::<usize>()? - 1, cap[2].to_owned());
    }

    if result.len() > 0 {
        return Ok(result);
    }

    let one_pack = Regex::new(r"Buildpack URL\n(.+)").unwrap();
    let mut capture = one_pack.captures_iter(&output);
    if let Some(cap) = capture.next() {
        Ok(vec![cap[1].to_string()])
    } else {
        Ok(Vec::new())
    }
}

fn next_match_action<'t, T: PartialEq>(from: &'t [T], to: &'t [T]) -> Option<Action<&'t T>> {
    match (from, to) {
        ([], []) => None,
        ([first_from, ..], [first_to, ..]) if first_from != first_to => Some(Action::Remove(0)),
        ([_, ..], []) => Some(Action::Remove(0)),
        ([], [first_to, ..]) => Some(Action::Insert(0, first_to)),
        ([_, from @ ..], [_, to @ ..]) => next_match_action(from, to).map(|a| a.add_one()),
    }
}

#[derive(Debug)]
enum Action<T> {
    Remove(usize),
    Insert(usize, T),
}

impl<T> Action<T> {
    fn add_one(self) -> Self {
        match self {
            Action::Remove(i) => Action::Remove(i + 1),
            Action::Insert(i, t) => Action::Insert(i + 1, t),
        }
    }
}

fn copy_files_to_root(sub_dir: &Path, files: &[PathBuf]) -> anyhow::Result<()> {
    for file in files {
        run(format!("cp {} .", sub_dir.join(file).display()))?;
    }
    Ok(())
}

fn create_deploy_branch(config: &Config) -> anyhow::Result<String> {
    let current = run("git rev-parse --abbrev-ref HEAD")?;

    let branch = format!("sanity/heroku/{}", config.app);

    let _ = run(format!("git branch -D {}", branch));
    run(format!("git checkout -b {}", branch))?;

    run("git add .")?;
    run(format!("git commit -m 'Deploy to heroku/{}'", config.app))?;
    run(format!("git checkout {}", current))?;

    Ok(branch)
}
