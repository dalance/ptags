use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::str;
use std::io::{BufReader, Read, Write};
use std::thread;
use std::sync::mpsc;
use bin::Opt;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        FromUtf8Error(::std::str::Utf8Error);
        FromRecvError(::std::sync::mpsc::RecvError);
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
        let _ = Command::new(&opt.bin_ctags)
            .arg(format!("-L {}", tmp_empty.path().to_string_lossy()))
            .arg(format!("-f {}", tmp_tags.path().to_string_lossy()))
            .args(&opt.opt_ctags)
            .current_dir(&opt.dir)
            .status();
        let mut f = BufReader::new(tmp_tags);
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
    }

    fn get_cmd(opt: &Opt, args: &[String]) -> Result<String> {
        let mut cmd = format!("{}", opt.bin_ctags.to_string_lossy());
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
            iter.next().unwrap(),
            "BIN_NAME\tMakefile\t/^BIN_NAME = ptags$/;\"\tm"
        );
    }

    #[test]
    fn test_call_with_opt() {
        let args = vec!["ptags", "-t", "1", "-c=-u"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files).unwrap();
        let mut iter = str::from_utf8(&outputs[0].stdout).unwrap().lines();
        assert_eq!(
            iter.next().unwrap(),
            "VERSION\tMakefile\t/^VERSION = $(patsubst \"%\",%, $(word 3, $(shell grep version Cargo.toml)))$/;\"\tm"
        );
    }

    #[test]
    fn test_command_fail() {
        let args = vec!["ptags", "--bin-ctags", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files);
        assert_eq!(format!("{:?}", outputs), "Err(Error(CommandFailed(\"aaa\", Error { repr: Os { code: 2, message: \"No such file or directory\" } }), State { next_error: None, backtrace: None }))");
    }

    #[test]
    fn test_ctags_fail() {
        let args = vec!["ptags", "--opt-ctags=--u"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = CmdCtags::call(&opt, &files);
        assert_eq!(format!("{:?}", outputs), "Err(Error(CtagsFailed(\"ctags -L - -f - --u\", \"\"), State { next_error: None, backtrace: None }))");
    }

    #[test]
    fn test_get_tags_header() {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let output = CmdCtags::get_tags_header(&opt).unwrap();
        assert_eq!(
            output.lines().next().unwrap(),
            "!_TAG_FILE_FORMAT\t2\t/extended format; --format=1 will not append ;\" to lines/"
        );
    }
}
