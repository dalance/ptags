use bin::Opt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
        Utf8(::std::str::Utf8Error);
    }
    errors {
        GitFailed(cmd: String, err: String) {
            description("git failed")
            display("git failed: {}\n{}", cmd, err)
        }
        CommandFailed(path: PathBuf, err: ::std::io::Error) {
            description("git command failed")
            display("git command \"{}\" failed: {}", path.to_string_lossy(), err)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// CmdGit
// ---------------------------------------------------------------------------------------------------------------------

pub struct CmdGit;

impl CmdGit {
    pub fn get_files(opt: &Opt) -> Result<Vec<String>> {
        let mut list = CmdGit::ls_files(&opt)?;
        if opt.exclude_lfs {
            let lfs_list = CmdGit::lfs_ls_files(&opt)?;
            let mut new_list = Vec::new();
            for l in list {
                if !lfs_list.contains(&l) {
                    new_list.push(l);
                }
            }
            list = new_list;
        }
        Ok(list)
    }

    fn call(opt: &Opt, args: &[String]) -> Result<Output> {
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

    fn ls_files(opt: &Opt) -> Result<Vec<String>> {
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

        if opt.verbose {
            eprintln!("Files: {}", ret.len());
        }

        Ok(ret)
    }

    fn lfs_ls_files(opt: &Opt) -> Result<Vec<String>> {
        let mut args = vec![String::from("lfs"), String::from("ls-files")];
        args.append(&mut opt.opt_git_lfs.clone());

        let output = CmdGit::call(&opt, &args)?;

        let cdup = CmdGit::show_cdup(&opt)?;
        let prefix = CmdGit::show_prefix(&opt)?;

        let list = str::from_utf8(&output.stdout)?.lines();
        let mut ret = Vec::new();
        for l in list {
            let mut path = String::from(l.split(' ').nth(2).unwrap_or(""));
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

    fn get_cmd(opt: &Opt, args: &[String]) -> Result<String> {
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
    use bin::Opt;
    use std::fs;
    use std::io::{BufWriter, Write};
    use structopt::StructOpt;

    static TRACKED_FILES: [&'static str; 19] = [
        ".cargo/config",
        ".gitattributes",
        ".gitignore",
        ".gitmodules",
        ".travis.yml",
        "Cargo.lock",
        "Cargo.toml",
        "LICENSE",
        "Makefile",
        "README.md",
        "appveyor.yml",
        "benches/ptags_bench.rs",
        "src/bin.rs",
        "src/cmd_ctags.rs",
        "src/cmd_git.rs",
        "src/lib.rs",
        "src/main.rs",
        "test/lfs.txt",
        "test/ptags_test",
    ];

    #[test]
    fn test_get_files() {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::get_files(&opt).unwrap();
        assert_eq!(files, TRACKED_FILES,);
    }

    #[test]
    fn test_get_files_exclude_lfs() {
        let args = vec!["ptags", "--exclude-lfs"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::get_files(&opt).unwrap();

        let mut expect_files = Vec::new();
        expect_files.extend_from_slice(&TRACKED_FILES);
        let idx = expect_files.binary_search(&"test/lfs.txt").unwrap();
        expect_files.remove(idx);

        assert_eq!(files, expect_files,);
    }

    #[test]
    fn test_get_files_exclude_lfs_cd() {
        let args = vec!["ptags", "--exclude-lfs", "src"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::get_files(&opt).unwrap();
        assert_eq!(
            files,
            vec!["bin.rs", "cmd_ctags.rs", "cmd_git.rs", "lib.rs", "main.rs"]
        );
    }

    #[test]
    fn test_get_files_include_submodule() {
        let args = vec!["ptags", "--include-submodule"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::get_files(&opt).unwrap();

        let mut expect_files = Vec::new();
        expect_files.extend_from_slice(&TRACKED_FILES);
        let idx = expect_files.binary_search(&"test/ptags_test").unwrap();
        expect_files.remove(idx);
        expect_files.push("test/ptags_test/README.md");

        assert_eq!(files, expect_files,);
    }

    #[test]
    fn test_get_files_include_untracked() {
        {
            let mut f = BufWriter::new(fs::File::create("tmp").unwrap());
            let _ = f.write(b"");
        }
        let args = vec!["ptags", "--include-untracked"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::get_files(&opt).unwrap();
        let _ = fs::remove_file("tmp");

        let mut expect_files = Vec::new();
        expect_files.extend_from_slice(&TRACKED_FILES);
        expect_files.push("tmp");

        assert_eq!(files, expect_files,);
    }

    #[test]
    fn test_command_fail() {
        let args = vec!["ptags", "--bin-git", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::ls_files(&opt);
        assert_eq!(
            &format!("{:?}", files)[0..68],
            "Err(Error(CommandFailed(\"aaa\", Error { repr: Os { code: 2, message: "
        );
    }

    #[test]
    fn test_git_fail() {
        let args = vec!["ptags", "--opt-git=-aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = CmdGit::ls_files(&opt);
        assert_eq!(
            &format!("{:?}", files)[0..83],
            "Err(Error(GitFailed(\"git ls-files --cached --exclude-standard -aaa\", \"error: unknow"
        );
    }

}
