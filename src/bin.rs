use crate::cmd_ctags::CmdCtags;
use crate::cmd_git::CmdGit;
use dirs;
use failure::{Error, ResultExt};
use serde_derive::{Deserialize, Serialize};
use std::fs;
use std::io::{stdout, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::Output;
use std::str;
use structopt::{clap, StructOpt};
use structopt_toml::StructOptToml;
use time::PreciseTime;
use toml;

// ---------------------------------------------------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, StructOpt, StructOptToml)]
#[serde(default)]
#[structopt(name = "ptags")]
#[structopt(raw(
    long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"
))]
#[structopt(raw(setting = "clap::AppSettings::AllowLeadingHyphen"))]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
pub struct Opt {
    /// Number of threads
    #[structopt(short = "t", long = "thread", default_value = "8")]
    pub thread: usize,

    /// Output filename ( filename '-' means output to stdout )
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
    #[structopt(short = "c", long = "opt-ctags", raw(number_of_values = "1"))]
    pub opt_ctags: Vec<String>,

    /// Options passed to git
    #[structopt(short = "g", long = "opt-git", raw(number_of_values = "1"))]
    pub opt_git: Vec<String>,

    /// Options passed to git-lfs
    #[structopt(long = "opt-git-lfs", raw(number_of_values = "1"))]
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

    /// Disable tags sort
    #[structopt(long = "unsorted")]
    pub unsorted: bool,

    /// Glob pattern of exclude file ( ex. --exclude '*.rs' )
    #[structopt(short = "e", long = "exclude", raw(number_of_values = "1"))]
    pub exclude: Vec<String>,

    /// Generate shell completion file
    #[structopt(
        long = "completion",
        raw(possible_values = "&[\"bash\", \"fish\", \"zsh\", \"powershell\"]")
    )]
    pub completion: Option<String>,

    /// Generate configuration sample file
    #[structopt(long = "config")]
    pub config: bool,
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

pub fn git_files(opt: &Opt) -> Result<Vec<String>, Error> {
    let list = CmdGit::get_files(&opt)?;
    let mut files = vec![String::from(""); opt.thread];

    for (i, f) in list.iter().enumerate() {
        files[i % opt.thread].push_str(f);
        files[i % opt.thread].push_str("\n");
    }

    Ok(files)
}

fn call_ctags(opt: &Opt, files: &[String]) -> Result<Vec<Output>, Error> {
    Ok(CmdCtags::call(&opt, &files)?)
}

fn get_tags_header(opt: &Opt) -> Result<String, Error> {
    Ok(CmdCtags::get_tags_header(&opt).context("failed to get ctags header")?)
}

fn write_tags(opt: &Opt, outputs: &[Output]) -> Result<(), Error> {
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

    let mut f = if opt.output.to_str().unwrap_or("") == "-" {
        BufWriter::new(Box::new(stdout()) as Box<dyn Write>)
    } else {
        let f = fs::File::create(&opt.output)?;
        BufWriter::new(Box::new(f) as Box<dyn Write>)
    };

    f.write(get_tags_header(&opt)?.as_bytes())?;

    while lines.iter().any(|x| x.is_some()) {
        let mut min = 0;
        for i in 1..lines.len() {
            if opt.unsorted {
                if !lines[i].is_none() && lines[min].is_none() {
                    min = i;
                }
            } else {
                if !lines[i].is_none()
                    && (lines[min].is_none() || lines[i].unwrap() < lines[min].unwrap())
                {
                    min = i;
                }
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

pub fn run_opt(opt: &Opt) -> Result<(), Error> {
    if opt.config {
        let toml = toml::to_string(&opt)?;
        println!("{}", toml);
        return Ok(());
    }

    match opt.completion {
        Some(ref x) => {
            let shell = match x.as_str() {
                "bash" => clap::Shell::Bash,
                "fish" => clap::Shell::Fish,
                "zsh" => clap::Shell::Zsh,
                "powershell" => clap::Shell::PowerShell,
                _ => clap::Shell::Bash,
            };
            Opt::clap().gen_completions("ptags", shell, "./");
            return Ok(());
        }
        None => {}
    }

    let files;
    let time_git_files = watch_time!({
        files = git_files(&opt).context("failed to get file list")?;
    });

    let outputs;
    let time_call_ctags = watch_time!({
        outputs = call_ctags(&opt, &files).context("failed to call ctags")?;
    });

    let time_write_tags = watch_time!({
        let _ = write_tags(&opt, &outputs)
            .context(format!("failed to write file ({:?})", &opt.output))?;
    });

    if opt.stat {
        let sum: usize = files.iter().map(|x| x.lines().count()).sum();

        eprintln!("\nStatistics");
        eprintln!("- Options");
        eprintln!("    thread    : {}\n", opt.thread);

        eprintln!("- Searched files");
        eprintln!("    total     : {}\n", sum);

        eprintln!("- Elapsed time[ms]");
        eprintln!("    git_files : {}", time_git_files.num_milliseconds());
        eprintln!("    call_ctags: {}", time_call_ctags.num_milliseconds());
        eprintln!("    write_tags: {}", time_write_tags.num_milliseconds());
    }

    Ok(())
}

#[cfg_attr(tarpaulin, skip)]
pub fn run() -> Result<(), Error> {
    let cfg_path = match dirs::home_dir() {
        Some(mut path) => {
            path.push(".ptags.toml");
            if path.exists() {
                Some(path)
            } else {
                None
            }
        }
        None => None,
    };

    let opt = match cfg_path {
        Some(path) => {
            let mut f =
                fs::File::open(&path).context(format!("failed to open file ({:?})", path))?;
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            Opt::from_args_with_toml(&s).context(format!("failed to parse toml ({:?})", path))?
        }
        None => Opt::from_args(),
    };
    run_opt(&opt)
}

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_run() {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_run_opt() {
        let args = vec!["ptags", "-s", "-v", "--validate-utf8", "--unsorted"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_run_fail() {
        let args = vec!["ptags", "--bin-git", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert_eq!(
            &format!("{:?}", ret)[0..42],
            "Err(Os { code: 2, kind: NotFound, message:"
        );
    }

    #[test]
    fn test_run_completion() {
        let args = vec!["ptags", "--completion", "bash"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
        let args = vec!["ptags", "--completion", "fish"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
        let args = vec!["ptags", "--completion", "zsh"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
        let args = vec!["ptags", "--completion", "powershell"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());

        assert!(Path::new("ptags.bash").exists());
        assert!(Path::new("ptags.fish").exists());
        assert!(Path::new("_ptags").exists());
        assert!(Path::new("_ptags.ps1").exists());
        let _ = fs::remove_file("ptags.bash");
        let _ = fs::remove_file("ptags.fish");
        let _ = fs::remove_file("_ptags");
        let _ = fs::remove_file("_ptags.ps1");
    }

    #[test]
    fn test_run_config() {
        let args = vec!["ptags", "--config"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }
}
