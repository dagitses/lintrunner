use std::{collections::HashSet, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use structopt::StructOpt;

use lintrunner::{
    do_init, do_lint, lint_config::get_linters_from_config, path::AbsPath, render::print_error,
};

#[derive(Debug, StructOpt)]
#[structopt(name = "lintrunner", about = "A lint runner")]
struct Opt {
    #[structopt(short, long)]
    verbose: bool,

    /// Path to a toml file defining which linters to run
    #[structopt(long, default_value = ".lintrunner.toml")]
    config: String,

    /// If set, any suggested patches will be applied
    #[structopt(short, long)]
    apply_patches: bool,

    /// Shell command that returns new-line separated paths to lint
    /// (e.g. --paths-cmd 'git ls-files path/to/project')
    #[structopt(long)]
    paths_cmd: Option<String>,

    /// Comma-separated list of linters to skip (e.g. --skip CLANGFORMAT,NOQA")
    #[structopt(long)]
    skip: Option<String>,

    /// Comma-separated list of linters to run (opposite of --skip)
    #[structopt(long)]
    take: Option<String>,

    /// If set, lintrunner will render lint messages as JSON, according to the
    /// LintMessage spec.
    #[structopt(long)]
    json: bool,

    #[structopt(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(StructOpt, Debug)]
enum SubCommand {
    /// Perform first-time setup for linters
    Init {
        /// If set, do not actually execute initialization commands, just print them
        #[structopt(long, short)]
        dry_run: bool,
    },
}

fn do_main() -> Result<i32> {
    let opt = Opt::from_args();
    let log_level = match (opt.verbose, opt.json) {
        // Verbose overrides json
        (true, true) => log::LevelFilter::Debug,
        (true, false) => log::LevelFilter::Debug,
        // If just json is asked for, suppress most output except hard errors.
        (false, true) => log::LevelFilter::Error,
        // Default
        (false, false) => log::LevelFilter::Info,
    };
    env_logger::Builder::new().filter_level(log_level).init();

    let config_path = AbsPath::new(PathBuf::from(&opt.config))
        .with_context(|| format!("Could not read lintrunner config at: '{}'", opt.config))?;
    let skipped_linters = opt.skip.map(|linters| {
        linters
            .split(',')
            .map(|linter_name| linter_name.to_string())
            .collect::<HashSet<_>>()
    });
    let taken_linters = opt.take.map(|linters| {
        linters
            .split(',')
            .map(|linter_name| linter_name.to_string())
            .collect::<HashSet<_>>()
    });

    let linters = get_linters_from_config(&config_path, skipped_linters, taken_linters)?;

    let enable_spinners = !opt.verbose && !opt.json;

    match opt.cmd {
        Some(SubCommand::Init { dry_run }) => {
            // Just run initialization commands, don't actually lint.
            do_init(linters, dry_run)
        }
        None => {
            // Default command is to just lint.
            do_lint(
                linters,
                opt.paths_cmd,
                opt.apply_patches,
                opt.json,
                enable_spinners,
            )
        }
    }
}

fn main() {
    let code = match do_main() {
        Ok(code) => code,
        Err(err) => {
            print_error(&err)
                .context("failed to print exit error")
                .unwrap();
            1
        }
    };

    // Flush the output before exiting, in case there is anything left in the buffers.
    drop(std::io::stdout().flush());
    drop(std::io::stderr().flush());

    // exit() abruptly ends the process while running no destructors. We should
    // make sure that nothing is alive before running this.
    std::process::exit(code);
}
