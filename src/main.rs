#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate structopt;
extern crate tempfile;
extern crate time;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::str;
use std::io::{BufReader, BufWriter, Read, Write};
use std::thread;
use std::sync::mpsc;
use structopt::StructOpt;
use tempfile::NamedTempFile;
use time::PreciseTime;

// ---------------------------------------------------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------------------------------------------------

#[derive(StructOpt, Debug)]
#[structopt(name = "ptags")]
struct Opt {
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
}

// ---------------------------------------------------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        FromUtf8Error(::std::str::Utf8Error);
        FromRecvError(::std::sync::mpsc::RecvError);
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
    if opt.verbose {
        eprint!(
            "Call : {} ls-files --cached --other --exclude-standard ",
            opt.git_bin.to_string_lossy()
        );
        for o in &opt.git_opt {
            eprint!("{} ", o);
        }
        eprintln!("");
    }

    let output = Command::new(&opt.git_bin)
        .arg("ls-files")
        .arg("--cached")
        .arg("--other")
        .arg("--exclude-standard")
        .args(&opt.git_opt)
        .current_dir(&opt.dir)
        .output()?;

    let list = str::from_utf8(&output.stdout)?.lines();
    let mut files = vec![String::from(""); opt.thread];

    for (i, f) in list.enumerate() {
        files[i % opt.thread].push_str(&format!("{}\n", f));
    }

    Ok(files)
}

fn call_ctags(opt: &Opt, files: &Vec<String>) -> Result<Vec<Output>> {
    let (tx, rx) = mpsc::channel();

    for i in 0..opt.thread {
        let tx = tx.clone();
        let file = files[i].clone();
        let dir = opt.dir.clone();
        let ctags_bin = opt.ctags_bin.clone();
        let ctags_opt = opt.ctags_opt.clone();

        if opt.verbose {
            eprint!("Call : {} -L - -f - ", opt.ctags_bin.to_string_lossy());
            for o in &opt.ctags_opt {
                eprint!("{} ", o);
            }
            eprintln!("");
        }

        thread::spawn(move || {
            let child = Command::new(ctags_bin)
                .arg("-L -")
                .arg("-f -")
                .args(ctags_opt)
                .current_dir(dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn();
            match child {
                Ok(mut x) => {
                    {
                        let stdin = x.stdin.as_mut().unwrap();
                        let _ = stdin.write(file.as_bytes());
                    }
                    let _ = tx.send(Ok(x));
                }
                Err(x) => {
                    let _ = tx.send(Err(x));
                }
            }
        });
    }

    let mut children = Vec::new();
    for _ in 0..opt.thread {
        children.push(rx.recv());
    }

    let mut output = Vec::new();
    for child in children {
        output.push(child??.wait_with_output()?);
    }

    Ok(output)
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

fn get_tags_header(opt: &Opt) -> Result<String> {
    let tmp_empty = NamedTempFile::new()?;
    let tmp_tags = NamedTempFile::new()?;
    let _ = Command::new(&opt.ctags_bin)
        .arg(format!("-L {}", tmp_empty.path().to_string_lossy()))
        .arg(format!("-f {}", tmp_tags.path().to_string_lossy()))
        .args(&opt.ctags_opt)
        .current_dir(&opt.dir)
        .status();
    let mut f = BufReader::new(tmp_tags);
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
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

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_files() {
        let args = vec!["ptags", "-t", "4"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        assert_eq!(
            files,
            vec![
                ".gitignore\n",
                "Cargo.lock\n",
                "Cargo.toml\n",
                "src/main.rs\n",
            ]
        );
    }

    #[test]
    fn test_call_ctags() {
        let args = vec!["ptags", "-t", "1"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = call_ctags(&opt, &files).unwrap();
        let mut iter = str::from_utf8(&outputs[0].stdout).unwrap().lines();
        assert_eq!(
            iter.next().unwrap(),
            "Opt\tsrc/main.rs\t/^struct Opt {$/;\"\ts"
        );
    }

    #[test]
    fn test_git_files_fail() {
        let args = vec!["ptags", "--git-bin", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt);
        assert_eq!(format!("{:?}", files), "Err(Error(Io(Error { repr: Os { code: 2, message: \"No such file or directory\" } }), State { next_error: None, backtrace: None }))");
    }

    #[test]
    fn test_call_ctags_fail() {
        let args = vec!["ptags", "--ctags-bin", "aaa"];
        let opt = Opt::from_iter(args.iter());
        let files = git_files(&opt).unwrap();
        let outputs = call_ctags(&opt, &files);
        assert_eq!(format!("{:?}", outputs), "Err(Error(Io(Error { repr: Os { code: 2, message: \"No such file or directory\" } }), State { next_error: None, backtrace: None }))");
    }
}
