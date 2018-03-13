use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;
use super::Opt;

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
        FromUtf8Error(::std::str::Utf8Error);
    }
    errors {
        GitLsFailed(cmd: String, err: String) {
            display("git ls-files failed: {}\n{}", cmd, err)
        }
        GitNotFound(path: PathBuf, err: ::std::io::Error) {
            display("git command \"{}\" failed: {}", path.to_string_lossy(), err)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// CmdGit
// ---------------------------------------------------------------------------------------------------------------------

pub struct CmdGit;

impl CmdGit {
    pub fn ls_files(opt: &Opt) -> Result<Vec<String>> {
        let mut git_opt = opt.git_opt.clone();
        git_opt.push(String::from("--cached"));
        git_opt.push(String::from("--exclude-standard"));
        if opt.include_submodule {
            git_opt.push(String::from("--recurse-submodules"));
        } else if opt.include_untracked {
            git_opt.push(String::from("--other"));
        }

        let mut git_cmd = format!(
            "{} ls-files ",
            opt.git_bin.to_string_lossy()
        );
        for o in &git_opt {
            git_cmd = format!("{} {}", git_cmd, o);
        }
        git_cmd = format!("{} {}", git_cmd, opt.dir.to_string_lossy());
        if opt.verbose {
            eprintln!("Call : {}", git_cmd);
        }

        let output: Result<Output> = Command::new(&opt.git_bin)
            .arg("ls-files")
            .args(&git_opt)
            .current_dir(&opt.dir)
            .output()
            .or_else(|x| Err(ErrorKind::GitNotFound(opt.git_bin.clone(), x).into()));
        let output = output?;

        if !output.status.success() {
            bail!(ErrorKind::GitLsFailed(
                git_cmd,
                String::from(str::from_utf8(&output.stderr)?)
            ));
        }

        let list = str::from_utf8(&output.stdout)?.lines();
        let mut ret = Vec::new();
        for l in list {
            ret.push(String::from(l));
        }
        ret.sort();
        Ok(ret)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::CmdGit;
    use super::super::Opt;
    use structopt::StructOpt;

    #[test]
    fn test_ls_files() {
        let args = vec!["ptags", "-t", "9"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::ls_files(&opt).unwrap();
        assert_eq!(
            files,
            vec![
                ".cargo/config",
                ".gitignore",
                ".travis.yml",
                "Cargo.lock",
                "Cargo.toml",
                "LICENSE",
                "Makefile",
                "README.md",
                "src/cmd_ctags.rs",
                "src/cmd_git.rs",
                "src/main.rs",
            ]
        );
    }

    #[test]
    fn test_ls_files_fail() {
        let args = vec!["ptags", "--git-bin", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::ls_files(&opt);
        assert_eq!(format!("{:?}", files), "Err(Error(GitNotFound(\"aaa\", Error { repr: Os { code: 2, message: \"No such file or directory\" } }), State { next_error: None, backtrace: None }))");
    }

}
