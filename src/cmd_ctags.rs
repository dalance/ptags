use bin::Opt;
#[cfg(linux)]
use nix::fcntl::{fcntl, FcntlArg};
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Write};
#[cfg(linux)]
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Output, Stdio};
use std::str;
use std::sync::mpsc;
use std::thread;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Utf8(::std::str::Utf8Error);
        Recv(::std::sync::mpsc::RecvError);
        Nix(::nix::Error) #[cfg(linux)];
    }
    errors {
        CtagsFailed(cmd: String, err: String) {
            description("ctags failed")
            display("ctags failed: {}\n{}", cmd, err)
        }
        CommandFailed(path: PathBuf, err: ::std::io::Error) {
            description("ctags command failed")
            display("ctags command \"{}\" failed: {}", path.to_string_lossy(), err)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// CmdCtags
// ---------------------------------------------------------------------------------------------------------------------

pub struct CmdCtags;

impl CmdCtags {
    pub fn call(opt: &Opt, files: &[String]) -> Result<Vec<Output>> {
        let mut args = Vec::new();
        args.push(String::from("-L -"));
        args.push(String::from("-f -"));
        if opt.unsorted {
            args.push(String::from("--sort=no"));
        }
        for e in &opt.exclude {
            args.push(String::from(format!("--exclude={}", e)));
        }
        args.append(&mut opt.opt_ctags.clone());

        let cmd = CmdCtags::get_cmd(&opt, &args)?;

        let (tx, rx) = mpsc::channel::<Result<Output>>();

        for i in 0..opt.thread {
            let tx = tx.clone();
            let file = files[i].clone();
            let dir = opt.dir.clone();
            let bin_ctags = opt.bin_ctags.clone();
            let args = args.clone();

            if opt.verbose {
                eprintln!("Call : {}", cmd);
            }

            thread::spawn(move || {
                let child = Command::new(bin_ctags.clone())
                    .args(args)
                    .current_dir(dir)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    //.stderr(Stdio::piped()) // Stdio::piped is x2 slow to wait_with_output() completion
                    .stderr(Stdio::null())
                    .spawn();
                match child {
                    Ok(mut x) => {
                        {
                            let stdin = x.stdin.as_mut().unwrap();
                            let _ = CmdCtags::set_pipe_size(&stdin, file.len() as i32)
                                .or_else(|x| tx.send(Err(x.into())));
                            let _ = stdin.write(file.as_bytes());
                        }
                        match x.wait_with_output() {
                            Ok(mut x) => {
                                let _ = tx.send(Ok(x));
                            }
                            Err(x) => {
                                let _ = tx.send(Err(x.into()));
                            }
                        }
                    }
                    Err(x) => {
                        let _ = tx.send(Err(ErrorKind::CommandFailed(bin_ctags.clone(), x).into()));
                    }
                }
            });
        }

        let mut children = Vec::new();
        for _ in 0..opt.thread {
            children.push(rx.recv());
        }

        let mut outputs = Vec::new();
        for child in children {
            let output = child??;

            if !output.status.success() {
                bail!(ErrorKind::CtagsFailed(
                    cmd,
                    String::from(str::from_utf8(&output.stderr)?)
                ));
            }

            outputs.push(output);
        }

        Ok(outputs)
    }

    pub fn get_tags_header(opt: &Opt) -> Result<String> {
        let tmp_empty = NamedTempFile::new()?;
        let tmp_tags = NamedTempFile::new()?;
        let tmp_tags_path: PathBuf = tmp_tags.path().into();
        // In windiws environment, write access by ctags to the opened tmp_tags fails.
        // So the tmp_tags must be closed and deleted.
        tmp_tags.close()?;

        let _ = Command::new(&opt.bin_ctags)
            .arg(format!("-L {}", tmp_empty.path().to_string_lossy()))
            .arg(format!("-f {}", tmp_tags_path.to_string_lossy()))
            .args(&opt.opt_ctags)
            .current_dir(&opt.dir)
            .status();
        let mut f = BufReader::new(File::open(&tmp_tags_path)?);
        let mut s = String::new();
        f.read_to_string(&mut s)?;

        fs::remove_file(&tmp_tags_path)?;

        Ok(s)
    }

    fn get_cmd(opt: &Opt, args: &[String]) -> Result<String> {
        let mut cmd = format!("{}", opt.bin_ctags.to_string_lossy());
        for arg in args {
            cmd = format!("{} {}", cmd, arg);
        }
        Ok(cmd)
    }

    #[allow(dead_code)]
    fn is_exuberant_ctags(opt: &Opt) -> Result<bool> {
        let output = Command::new(&opt.bin_ctags)
            .arg("--version")
            .current_dir(&opt.dir)
            .output()?;
        Ok(str::from_utf8(&output.stdout)?.starts_with("Exuberant Ctags"))
    }

    #[cfg(linux)]
    fn set_pipe_size(stdin: &ChildStdin, len: i32) -> Result<()> {
        fcntl(stdin.as_raw_fd(), FcntlArg::F_SETPIPE_SZ(len))?;
        Ok(())
    }

    #[cfg(not(linux))]
    fn set_pipe_size(_stdin: &ChildStdin, _len: i32) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::bin::{git_files, Opt};
    use super::CmdCtags;
    use std::str;
    use structopt::StructOpt;

    #[test]
    fn test_call() {
        let args = vec!["ptags", "-t", "1"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files).unwrap();
        let mut iter = str::from_utf8(&outputs[0].stdout).unwrap().lines();
        assert_eq!(
            iter.next().unwrap_or(""),
            "BIN_NAME\tMakefile\t/^BIN_NAME = ptags$/;\"\tm"
        );
    }

    #[test]
    fn test_call_with_opt() {
        let args = vec!["ptags", "-t", "1", "--opt-ctags=-u"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files).unwrap();
        let mut iter = str::from_utf8(&outputs[0].stdout).unwrap().lines();
        assert_eq!(
            iter.next().unwrap_or(""),
            "VERSION\tMakefile\t/^VERSION = $(patsubst \"%\",%, $(word 3, $(shell grep version Cargo.toml)))$/;\"\tm"
        );
    }

    #[test]
    fn test_call_exclude() {
        let args = vec!["ptags", "-t", "1", "--exclude=Make*", "-v"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files).unwrap();
        let mut iter = str::from_utf8(&outputs[0].stdout).unwrap().lines();

        // Exuberant Ctags doesn't support Rust ( *.rs ).
        // So the result becomes empty when 'Makefile' is excluded.
        if CmdCtags::is_exuberant_ctags(&opt).unwrap() {
            assert_eq!(iter.next().unwrap_or(""), "");
        } else {
            assert_eq!(
                iter.next().unwrap_or(""),
                "CmdCtags\tsrc/cmd_ctags.rs\t/^impl CmdCtags {$/;\"\tc"
            );
        }
    }

    #[test]
    fn test_command_fail() {
        let args = vec!["ptags", "--bin-ctags", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files);
        assert_eq!(
            &format!("{:?}", outputs)[0..68],
            "Err(Error(CommandFailed(\"aaa\", Error { repr: Os { code: 2, message: "
        );
    }

    #[test]
    fn test_ctags_fail() {
        let args = vec!["ptags", "--opt-ctags=--u"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files);
        assert_eq!(
            &format!("{:?}", outputs)[0..74],
            "Err(Error(CtagsFailed(\"ctags -L - -f - --u\", \"\"), State { next_error: None"
        );
    }

    #[test]
    fn test_get_tags_header() {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let output = CmdCtags::get_tags_header(&opt).unwrap();
        let output = output.lines().next();
        assert_eq!(
            output.unwrap_or(""),
            "!_TAG_FILE_FORMAT\t2\t/extended format; --format=1 will not append ;\" to lines/"
        );
    }
}
