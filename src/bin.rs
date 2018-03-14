use cmd_ctags::CmdCtags;
use cmd_git::CmdGit;
use std::fs;
use std::path::PathBuf;
use std::process::Output;
use std::str;
use std::io::{BufWriter, Write};
use structopt::StructOpt;
use time::PreciseTime;

// ---------------------------------------------------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------------------------------------------------

#[derive(StructOpt, Debug)]
#[structopt(name = "ptags")]
pub struct Opt {
    /// Number of threads
    #[structopt(short = "t", long = "thread", default_value = "8")]
    pub thread: usize,

    /// Output filename
    #[structopt(short = "f", long = "file", default_value = "tags", parse(from_os_str))]
    pub output: PathBuf,

    /// Search directory
    #[structopt(name = "DIR", default_value = ".", parse(from_os_str))]
    pub dir: PathBuf,

    /// Show statistics
    #[structopt(short = "s", long = "stat")]
    pub stat: bool,

    /// Path to ctags binary
    #[structopt(long = "bin-ctags", default_value = "ctags", parse(from_os_str))]
    pub bin_ctags: PathBuf,

    /// Path to git binary
    #[structopt(long = "bin-git", default_value = "git", parse(from_os_str))]
    pub bin_git: PathBuf,

    /// Options passed to ctags
    #[structopt(short = "c", long = "opt-ctags")]
    pub opt_ctags: Vec<String>,

    /// Options passed to git
    #[structopt(short = "g", long = "opt-git")]
    pub opt_git: Vec<String>,

    /// Options passed to git-lfs
    #[structopt(long = "opt-git-lfs")]
    pub opt_git_lfs: Vec<String>,

    /// Verbose mode
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,

    /// Exclude git-lfs tracked files
    #[structopt(long = "exclude-lfs")]
    pub exclude_lfs: bool,

    /// Include untracked files
    #[structopt(long = "include-untracked")]
    pub include_untracked: bool,

    /// Include ignored files
    #[structopt(long = "include-ignored")]
    pub include_ignored: bool,

    /// Include submodule files
    #[structopt(long = "include-submodule")]
    pub include_submodule: bool,

    /// Validate UTF8 sequence of tag file
    #[structopt(long = "validate-utf8")]
    pub validate_utf8: bool,
}

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    links {
        GitError(super::cmd_git::Error, super::cmd_git::ErrorKind);
        CtagsError(super::cmd_ctags::Error, super::cmd_ctags::ErrorKind);
    }
    foreign_links {
        Io(::std::io::Error);
        FromUtf8Error(::std::str::Utf8Error);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------------------------------------------------

macro_rules! watch_time (
    ( $func:block ) => (
        {
            let beg = PreciseTime::now();
            $func;
            beg.to(PreciseTime::now())
        }
    );
);

pub fn git_files(opt: &Opt) -> Result<Vec<String>> {
    let list = CmdGit::get_files(&opt)?;
    let mut files = vec![String::from(""); opt.thread];

    for (i, f) in list.iter().enumerate() {
        files[i % opt.thread].push_str(f);
        files[i % opt.thread].push_str("\n");
    }

    Ok(files)
}

fn call_ctags(opt: &Opt, files: &[String]) -> Result<Vec<Output>> {
    Ok(CmdCtags::call(&opt, &files)?)
}

fn get_tags_header(opt: &Opt) -> Result<String> {
    Ok(CmdCtags::get_tags_header(&opt)?)
}

fn write_tags(opt: &Opt, outputs: &[Output]) -> Result<()> {
    let mut iters = Vec::new();
    let mut lines = Vec::new();
    for o in outputs {
        let mut iter = if opt.validate_utf8 {
            str::from_utf8(&o.stdout)?.lines()
        } else {
            unsafe { str::from_utf8_unchecked(&o.stdout).lines() }
        };
        lines.push(iter.next());
        iters.push(iter);
    }

    let mut f = BufWriter::new(fs::File::create(&opt.output)?);

    f.write(get_tags_header(&opt)?.as_bytes())?;

    while lines.iter().any(|x| x.is_some()) {
        let mut min = 0;
        for i in 1..lines.len() {
            if !lines[i].is_none()
                && (lines[min].is_none() || lines[i].unwrap() < lines[min].unwrap())
            {
                min = i;
            }
        }
        f.write(lines[min].unwrap().as_bytes())?;
        f.write("\n".as_bytes())?;
        lines[min] = iters[min].next();
    }

    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------------------------------------------------

pub fn run_opt(opt: &Opt) -> Result<()> {
    let files;
    let time_git_files = watch_time!({
        files = git_files(&opt)?;
    });

    let outputs;
    let time_call_ctags = watch_time!({
        outputs = call_ctags(&opt, &files)?;
    });

    let time_write_tags = watch_time!({
        let _ = write_tags(&opt, &outputs)?;
    });

    if opt.stat {
        let sum: usize = files.iter().map(|x| x.lines().count()).sum();

        println!("\nStatistics");
        println!("- Options");
        println!("    thread    : {}\n", opt.thread);

        println!("- Searched files");
        println!("    total     : {}\n", sum);

        println!("- Elapsed time[ms]");
        println!("    git_files : {}", time_git_files.num_milliseconds());
        println!("    call_ctags: {}", time_call_ctags.num_milliseconds());
        println!("    write_tags: {}", time_write_tags.num_milliseconds());
    }

    Ok(())
}

pub fn run() -> Result<()> {
    let opt = Opt::from_args();
    run_opt(&opt)
}

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_run_opt() {
        let args = vec!["ptags", "-s", "-v", "--validate-utf8"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_run_fail() {
        let args = vec!["ptags", "--bin-git", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert_eq!(format!("{:?}", ret), "Err(Error(GitError(CommandFailed(\"aaa\", Error { repr: Os { code: 2, message: \"No such file or directory\" } })), State { next_error: None, backtrace: None }))");
    }
}
