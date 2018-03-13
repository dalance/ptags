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
        GitFailed(cmd: String, err: String) {
            display("git failed: {}\n{}", cmd, err)
        }
        CommandFailed(path: PathBuf, err: ::std::io::Error) {
            display("command \"{}\" failed: {}", path.to_string_lossy(), err)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// CmdGit
// ---------------------------------------------------------------------------------------------------------------------

pub struct CmdGit;

impl CmdGit {
    fn call(opt: &Opt, args: &Vec<String>) -> Result<Output> {
        let cmd = CmdGit::get_cmd(&opt, &args)?;
        if opt.verbose {
            eprintln!("Call : {}", cmd);
        }

        let output: Result<Output> = Command::new(&opt.bin_git)
            .args(args)
            .current_dir(&opt.dir)
            .output()
            .or_else(|x| Err(ErrorKind::CommandFailed(opt.bin_git.clone(), x).into()));
        let output = output?;

        if !output.status.success() {
            bail!(ErrorKind::GitFailed(
                cmd,
                String::from(str::from_utf8(&output.stderr)?)
            ));
        }

        Ok(output)
    }

    pub fn ls_files(opt: &Opt) -> Result<Vec<String>> {
        let mut args = vec![String::from("ls-files")];
        args.push(String::from("--cached"));
        args.push(String::from("--exclude-standard"));
        if opt.include_submodule {
            args.push(String::from("--recurse-submodules"));
        } else if opt.include_untracked {
            args.push(String::from("--other"));
        }
        args.append(&mut opt.opt_git.clone());

        let output = CmdGit::call(&opt, &args)?;

        let list = str::from_utf8(&output.stdout)?.lines();
        let mut ret = Vec::new();
        for l in list {
            ret.push(String::from(l));
        }
        ret.sort();
        Ok(ret)
    }

    pub fn lfs_ls_files(opt: &Opt) -> Result<Vec<String>> {
        let mut args = vec![String::from("lfs"), String::from("ls-files")];
        args.append(&mut opt.opt_git_lfs.clone());

        let output = CmdGit::call(&opt, &args)?;

        let cdup = CmdGit::show_cdup(&opt)?;
        let prefix = CmdGit::show_prefix(&opt)?;

        let list = str::from_utf8(&output.stdout)?.lines();
        let mut ret = Vec::new();
        for l in list {
            let mut path = String::from(l.split(" - ").nth(1).unwrap_or(""));
            if path.starts_with(&prefix) {
                path = path.replace(&prefix, "");
            } else {
                path = format!("{}{}", cdup, path);
            }
            ret.push(path);
        }
        ret.sort();
        Ok(ret)
    }

    fn show_cdup(opt: &Opt) -> Result<String> {
        let args = vec![String::from("rev-parse"), String::from("--show-cdup")];

        let output = CmdGit::call(&opt, &args)?;

        let mut list = str::from_utf8(&output.stdout)?.lines();
        Ok(String::from(list.next().unwrap_or("")))
    }

    fn show_prefix(opt: &Opt) -> Result<String> {
        let args = vec![String::from("rev-parse"), String::from("--show-prefix")];

        let output = CmdGit::call(&opt, &args)?;

        let mut list = str::from_utf8(&output.stdout)?.lines();
        Ok(String::from(list.next().unwrap_or("")))
    }

    fn get_cmd(opt: &Opt, args: &Vec<String>) -> Result<String> {
        let mut cmd = format!("{}", opt.bin_git.to_string_lossy());
        for arg in args {
            cmd = format!("{} {}", cmd, arg);
        }
        Ok(cmd)
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
        let args = vec!["ptags"];
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
    fn test_lfs_ls_files() {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::lfs_ls_files(&opt).unwrap();
        assert_eq!(
            files,
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_ls_files_fail() {
        let args = vec!["ptags", "--bin-git", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::ls_files(&opt);
        assert_eq!(format!("{:?}", files), "Err(Error(CommandFailed(\"aaa\", Error { repr: Os { code: 2, message: \"No such file or directory\" } }), State { next_error: None, backtrace: None }))");
    }

}
