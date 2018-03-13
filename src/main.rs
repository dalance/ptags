#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate structopt;
extern crate tempfile;
extern crate time;

mod cmd_git;
//mod cmd_git_lfs;
mod cmd_ctags;

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
    #[structopt(short = "t", long = "thread", default_value = "8")] thread: usize,

    #[structopt(short = "f", long = "file", default_value = "tags", parse(from_os_str))]
    output: PathBuf,

    #[structopt(name = "DIR", default_value = ".", parse(from_os_str))] dir: PathBuf,

    #[structopt(short = "s", long = "stat")] stat: bool,

    #[structopt(long = "ctags-bin", default_value = "ctags", parse(from_os_str))]
    ctags_bin: PathBuf,

    #[structopt(long = "git-bin", default_value = "git", parse(from_os_str))] git_bin: PathBuf,

    #[structopt(short = "c", long = "ctags-opt")] ctags_opt: Vec<String>,

    #[structopt(short = "g", long = "git-opt")] git_opt: Vec<String>,

    #[structopt(short = "v", long = "verbose")] verbose: bool,

    #[structopt(long = "exclude-lfs")] exclude_lfs: bool,

    #[structopt(long = "include-untracked")] include_untracked: bool,

    #[structopt(long = "include-ignored")] include_ignored: bool,

    #[structopt(long = "include-submodule")] include_submodule: bool,

}

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    links {
        GitError(cmd_git::Error, cmd_git::ErrorKind);
        CtagsError(cmd_ctags::Error, cmd_ctags::ErrorKind);
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

fn git_files(opt: &Opt) -> Result<Vec<String>> {
    let list = CmdGit::ls_files(&opt)?;
    let mut files = vec![String::from(""); opt.thread];

    for (i, f) in list.iter().enumerate() {
        files[i % opt.thread].push_str(&format!("{}\n", f));
    }

    Ok(files)
}

fn call_ctags(opt: &Opt, files: &Vec<String>) -> Result<Vec<Output>> {
    Ok(CmdCtags::call(&opt, &files)?)
}

fn get_tags_header(opt: &Opt) -> Result<String> {
    Ok(CmdCtags::get_tags_header(&opt)?)
}

fn write_tags(opt: &Opt, outputs: &Vec<Output>) -> Result<()> {
    let mut iters = Vec::new();
    let mut lines = Vec::new();
    for o in outputs {
        let mut iter = str::from_utf8(&o.stdout)?.lines();
        lines.push(iter.next());
        iters.push(iter);
    }

    let mut f = BufWriter::new(fs::File::create(&opt.output)?);

    f.write(get_tags_header(&opt)?.as_bytes())?;

    while lines.iter().any(|x| x.is_some()) {
        let (mut min_index, mut min_line) = (0, lines[0]);
        for i in 0..lines.len() {
            if !lines[i].is_none() && (min_line.is_none() || lines[i] < min_line) {
                min_index = i;
                min_line = lines[i];
            }
        }
        lines[min_index] = iters[min_index].next();
        f.write(format!("{}\n", min_line.unwrap_or("")).as_bytes())?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------------------------------------------------

fn run() -> Result<()> {
    let opt = Opt::from_args();

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

quick_main!(run);

