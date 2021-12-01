use regex::Regex;
use serde::*;
use std::{
    collections::HashSet,
    hash::Hash,
    path::{Path, PathBuf},
};

use crate::utils::run;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    buildpacks: Vec<String>,

    app: String,

    #[serde(default)]
    copy_to_root: Vec<PathBuf>,

    #[serde(default)]
    addons: HashSet<String>,
}

pub fn deploy(path: &Path, config: Config) -> anyhow::Result<()> {
    match_buildpacks(&config)?;
    match_addons(&config)?;
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
    while let Some(action) = next_match_action(&current_buildpacks(config)?, &config.buildpacks) {
        match action {
            ListAction::Remove(i) => {
                run(format!(
                    "heroku buildpacks:remove --app {} --index {}",
                    config.app,
                    i + 1
                ))?;
            }
            ListAction::Insert(i, v) => {
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

fn next_match_action<'t, T: PartialEq>(from: &'t [T], to: &'t [T]) -> Option<ListAction<&'t T>> {
    match (from, to) {
        ([], []) => None,
        ([first_from, ..], [first_to, ..]) if first_from != first_to => Some(ListAction::Remove(0)),
        ([_, ..], []) => Some(ListAction::Remove(0)),
        ([], [first_to, ..]) => Some(ListAction::Insert(0, first_to)),
        ([_, from @ ..], [_, to @ ..]) => next_match_action(from, to).map(|a| a.add_one()),
    }
}

#[derive(Debug)]
enum ListAction<T> {
    Remove(usize),
    Insert(usize, T),
}

impl<T> ListAction<T> {
    fn add_one(self) -> Self {
        match self {
            ListAction::Remove(i) => ListAction::Remove(i + 1),
            ListAction::Insert(i, t) => ListAction::Insert(i + 1, t),
        }
    }
}

fn match_addons(config: &Config) -> anyhow::Result<()> {
    while let Some(action) = next_match_set_action(&current_addons(config)?, &config.addons) {
        match action {
            SetAction::Add(addon) => {
                run(format!(
                    "heroku addons:create {addon} --app {app}",
                    addon = addon,
                    app = config.app
                ))?;
            }
            SetAction::Remove(addon) => {
                run(format!(
                    "heroku addons:detach {addon} --app {app}",
                    addon = addon,
                    app = config.app
                ))?;
            }
        }
    }
    Ok(())
}

fn current_addons(config: &Config) -> anyhow::Result<HashSet<String>> {
    let output = run(format!("heroku addons --app {} --json", config.app))?;
    let output_json = json::parse(&output)?;
    match output_json {
        json::JsonValue::Array(val) => Ok(val
            .into_iter()
            .map(|obj| {
                obj["addon_service"]["cli_plugin_name"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()),
        _ => {
            anyhow::bail!("Expected json array");
        }
    }
}

fn next_match_set_action<'t, T: Eq + Hash>(
    from: &'t HashSet<T>,
    to: &'t HashSet<T>,
) -> Option<SetAction<&'t T>> {
    to.difference(from)
        .next()
        .map(SetAction::Add)
        .or_else(|| from.difference(to).next().map(SetAction::Remove))
}

#[derive(Debug)]
enum SetAction<T> {
    Remove(T),
    Add(T),
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
