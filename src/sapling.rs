use crate::{log_utils, path, version_control};

use anyhow;
use anyhow::Context;

pub struct Repo { root: path::AbsPath }

impl version_control::System for Repo {
    fn new() -> anyhow::Result<Self> {
        let output = std::process::Command::new("sl")
            .arg("root")
            .output()?;
        anyhow::ensure!(output.status.success(), "Failed to determine Sapling root");
        let root = std::str::from_utf8(&output.stdout)?.trim();
        Ok(Repo { root: path::AbsPath::try_from(root)? })
    }

    fn get_head(&self) -> anyhow::Result<String> {
        let mut cmd = std::process::Command::new("sl");
        cmd.arg("whereami");
        let output = cmd.current_dir(&self.root).output()?;
        log_utils::ensure_output(&format!("{:?}", cmd), &output)?;
        let head = std::str::from_utf8(&output.stdout)?.trim();
        Ok(head.to_string())
    }

    fn get_merge_base_with(&self, merge_base_with: &str) -> anyhow::Result<String> {
        let output = std::process::Command::new("sl")
            .arg("log")
            .arg(format!("--rev=ancestor(., {})", merge_base_with))
            .arg("--template={node}")
            .current_dir(&self.root)
            .output()?;

        anyhow::ensure!(
            output.status.success(),
            format!("Failed to get most recent common ancestor between . and {merge_base_with}")
        );
        let merge_base = std::str::from_utf8(&output.stdout)?.trim();
        Ok(merge_base.to_string())
    }

    fn get_changed_files(&self, relative_to: Option<&str>) -> anyhow::Result<Vec<path::AbsPath>> {
        // Output of sl status looks like:
        // D    src/lib.rs
        // M    foo/bar.baz
        let re = regex::Regex::new(r"^[A-Z?]\s+")?;

        // Retrieve changed files in current commit.
        let mut cmd = std::process::Command::new("sl");
        cmd.arg("status");
        if let Some(relative_to) = relative_to {
            cmd.arg(format!("--rev={}", relative_to));
        }
        cmd.current_dir(&self.root);
        let output = cmd.output()?;
        log_utils::ensure_output(&format!("{:?}", cmd), &output)?;

        let commit_files_str = std::str::from_utf8(&output.stdout)?;

        let commit_files: std::collections::HashSet<String> = commit_files_str
            .split('\n')
            .map(|x| x.to_string())
            // Filter out deleted files.
            .filter(|line| !line.starts_with('D'))
            // Strip the status prefix.
            .map(|line| re.replace(&line, "").to_string())
            .filter(|line| !line.is_empty())
            .collect();

        log_utils::log_files("Linting commit diff files: ", &commit_files);

        commit_files.into_iter()
            // Git reports files relative to the root of git root directory, so retrieve
            // that and prepend it to the file paths.
            .map(|f| format!("{}", self.root.join(f).display()))
            .map(|f| {
                path::AbsPath::try_from(&f).with_context(|| {
                    format!("Failed to find file while gathering files to lint: {}", f)
                })
            })
            .collect::<anyhow::Result<_>>()
    }
}
