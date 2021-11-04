use std::{collections::HashSet, fs};

use crate::{linter::Linter, path::AbsPath};
use anyhow::{bail, Context, Result};
use glob::Pattern;
use log::debug;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct LintRunnerConfig {
    #[serde(rename = "linter")]
    linters: Vec<LintConfig>,
}

#[derive(Serialize, Deserialize)]
struct LintConfig {
    name: String,
    include_patterns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_patterns: Option<Vec<String>>,
    args: Vec<String>,
    init_args: Option<Vec<String>>,
    #[serde(default)]
    bypass_matched_file_filter: bool,
}

/// Given options specified by the user, return a list of linters to run.
pub fn get_linters_from_config(
    config_path: &AbsPath,
    skipped_linters: Option<HashSet<String>>,
    taken_linters: Option<HashSet<String>>,
) -> Result<Vec<Linter>> {
    let lint_runner_config = LintRunnerConfig::new(config_path)?;
    let mut linters = Vec::new();
    for lint_config in lint_runner_config.linters {
        let include_patterns = patterns_from_strs(&lint_config.include_patterns)?;
        let exclude_patterns = if let Some(exclude_patterns) = &lint_config.exclude_patterns {
            patterns_from_strs(exclude_patterns)?
        } else {
            Vec::new()
        };
        linters.push(Linter {
            name: lint_config.name,
            include_patterns,
            exclude_patterns,
            commands: lint_config.args,
            init_commands: lint_config.init_args,
            config_path: config_path.clone(),
            bypass_matched_file_filter: lint_config.bypass_matched_file_filter,
        });
    }
    debug!(
        "Found linters: {:?}",
        linters.iter().map(|l| &l.name).collect::<Vec<_>>()
    );

    // Apply --take
    if let Some(taken_linters) = taken_linters {
        debug!("Taking linters: {:?}", taken_linters);
        linters = linters
            .into_iter()
            .filter(|linter| taken_linters.contains(&linter.name))
            .collect();
    }

    // Apply --skip
    if let Some(skipped_linters) = skipped_linters {
        debug!("Skipping linters: {:?}", skipped_linters);
        linters = linters
            .into_iter()
            .filter(|linter| !skipped_linters.contains(&linter.name))
            .collect();
    }
    Ok(linters)
}

impl LintRunnerConfig {
    pub fn new(path: &AbsPath) -> Result<LintRunnerConfig> {
        let path = path.as_pathbuf();
        let lint_config = fs::read_to_string(path.as_path())
            .context(format!("Failed to read config file: '{}'.", path.display()))?;
        let config: LintRunnerConfig = toml::from_str(&lint_config).context(format!(
            "Config file '{}' had invalid schema",
            path.display()
        ))?;
        for linter in &config.linters {
            if let Some(init_args) = &linter.init_args {
                if init_args.iter().all(|arg| !arg.contains("{{DRYRUN}}")) {
                    bail!(
                        "Config for linter {} defines init args \
                         but does not take a {{{{DRYRUN}}}} argument.",
                        linter.name
                    );
                }
            }
        }

        Ok(config)
    }
}

fn patterns_from_strs(pattern_strs: &Vec<String>) -> Result<Vec<Pattern>> {
    pattern_strs
        .iter()
        .map(|pattern_str| {
            Pattern::new(pattern_str).map_err(|err| {
                anyhow::Error::msg(err)
                    .context("Could not parse pattern from linter configuration.")
            })
        })
        .collect::<Result<Vec<Pattern>>>()
}
