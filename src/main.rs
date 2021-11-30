use serde::*;
use std::path::Path;
use structopt::*;

mod heroku;
mod utils;

#[derive(StructOpt)]
struct Options {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    Deploy,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let options = Options::from_args();

    match options.command {
        Command::Deploy => {
            anyhow::ensure!(
                utils::run("git status --short")? == "",
                "Cannot deploy, working directory dirty"
            );

            ignore::Walk::new(".")
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|dir| is_sanity_dir(dir.path()))
                .try_for_each(|dir| deploy_dir(dir.path()))?;
        }
    }

    Ok(())
}

fn is_sanity_dir(path: &Path) -> bool {
    path.join("Sanityfile").exists()
        && match path.parent() {
            None => true,
            Some(par) => !is_sanity_dir(par),
        }
}

fn deploy_dir(path: &Path) -> anyhow::Result<()> {
    println!("Deploying '{}'", path.display());
    let sanityfile: Sanityfile = serde_yaml::from_slice(&std::fs::read(path.join("Sanityfile"))?)?;
    match sanityfile {
        Sanityfile::Heroku(config) => crate::heroku::deploy(path, config),
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum Sanityfile {
    Heroku(crate::heroku::Config),
}
