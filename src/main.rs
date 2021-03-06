use crate::ast_walker::AstWalker;
use crate::dir_walker::get_dir_walker;
use glob::Pattern;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use structopt::{clap::arg_enum, StructOpt};
use tracing::{info, warn};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

mod ast_walker;
mod dir_walker;

arg_enum! {
#[derive(Copy, Debug, Clone, Eq, PartialEq)]
pub enum Color {
    Auto,
    Always,
    Never,
}
}

#[derive(Clone, Debug, StructOpt)]
pub struct Config {
    #[structopt(long = "manifest-path")]
    manifest_path: Option<PathBuf>,
    #[structopt(long = "color", default_value = "auto")]
    color: Color,
    #[structopt(long = "exclude-files")]
    excluded_files: Vec<Pattern>,
}

pub fn get_analysis(root: PathBuf, excluded_files: &[Pattern]) {
    info!("Analysing project in {}", root.display());
    for e in get_dir_walker(root.clone()) {
        let relative = e.path().strip_prefix(&root).unwrap();
        if !excluded_files.iter().any(|x| x.matches_path(&relative)) {
            analyse_package(e.path(), &root);
        }
    }
}

/// Analyses a package of the target crate.
fn analyse_package(path: &Path, root: &Path) {
    if let Some(_file) = path.to_str() {
        let skip_cause_test = path.starts_with(root.join("tests"));
        let skip_cause_example = path.starts_with(root.join("examples"));
        if !(skip_cause_test || skip_cause_example) {
            if let Ok(walker) = AstWalker::new(path.to_path_buf()) {
                let bad_panics = walker.process();
                if !bad_panics.is_empty() {
                    warn!(
                        "Potentially undocumented panics in {}",
                        path.strip_prefix(root).unwrap().display()
                    );
                }
                for panik in &bad_panics {
                    println!("\t{}", panik);
                }
            }
        }
    }
}

pub fn setup_logging(color: Color) {
    let base_exceptions = |env: EnvFilter| {
        env.add_directive("doc_panic_checker=info".parse().unwrap())
            .add_directive(LevelFilter::INFO.into())
    };
    let filter = match std::env::var_os("RUST_LOG").map(|s| s.into_string()) {
        Some(Ok(env)) => {
            let mut filter = base_exceptions(EnvFilter::new(""));
            for s in env.split(',').into_iter() {
                match s.parse() {
                    Ok(d) => filter = filter.add_directive(d),
                    Err(err) => println!("WARN ignoring log directive: `{}`: {}", s, err),
                };
            }
            filter
        }
        _ => base_exceptions(EnvFilter::from_env("RUST_LOG")),
    };
    let with_colour = color != Color::Never;

    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::ERROR)
        .with_env_filter(filter)
        .with_ansi(with_colour)
        .with_target(false)
        .without_time()
        .init();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_args();
    setup_logging(config.color);

    if config
        .manifest_path
        .as_ref()
        .map(|x| x.file_name() != Some(OsStr::new("Cargo.toml")))
        .unwrap_or(false)
    {
        Err("The manifest-path must be a path to a Cargo.toml file")?;
    }

    let root = config
        .manifest_path
        .map(|x| x.canonicalize().ok())
        .flatten()
        .map(|x| x.parent().map(|x| x.to_path_buf()).unwrap_or_default())
        .unwrap_or_default();

    get_analysis(root, &config.excluded_files);
    Ok(())
}
