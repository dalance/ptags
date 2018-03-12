# ptags
A parallel [universal-ctags](https://ctags.io) wrapper for git repository

[![Build Status](https://travis-ci.org/dalance/ptags.svg?branch=master)](https://travis-ci.org/dalance/ptags)
[![Crates.io](https://img.shields.io/crates/v/ptags.svg)](https://crates.io/crates/ptags)

**ptags** is a [universal-ctags](https://ctags.io) wrapper to have the following features.
- Search git tracked files only ( `.gitignore` support )
- Call `ctags` command in parallel for acceleration

## Install
Download from [release page](https://github.com/dalance/ptags/releases/latest), and extract to the directory in PATH.

Alternatively you can install by [cargo](https://crates.io).

```
cargo install ptags
```

## Requirement

**ptags** uses `ctags` and `git` command.
The tested version is below.

| Command | Version                         |
| ------- | ------------------------------- |
| `ctags` | Universal Ctags 0.0.0(f9e6e3c1) |
|         | Exuberant Ctags 5.8             |
| `git`   | git version 2.14.2              |

## Usage

```
ptags 0.1.0
dalance@gmail.com
A parallel ctags wrapper for git repository

USAGE:
    ptags [FLAGS] [OPTIONS] [--] [DIR]

FLAGS:
    -h, --help       Prints help information
    -s, --stat
    -V, --version    Prints version information
    -v, --verbose

OPTIONS:
        --ctags-bin <ctags_bin>        [default: ctags]
    -c, --ctags-opt <ctags_opt>...
        --git-bin <git_bin>            [default: git]
    -g, --git-opt <git_opt>...
    -f, --file <output>                [default: tags]
    -t, --thread <thread>              [default: 8]

ARGS:
    <DIR>     [default: .]
```

You can pass options to `ctags` by`-c`/`--ctags_opt` option like below.

```
ptags -c='--exclude=aaa/*' -c='--exclude=bbb/*'
```

## Benchmark

### Environment
- CPU: Ryzen Threadripper 1950X
- MEM: 128GB
- OS : CentOS 7.4.1708

### Data
- https://github.com/torvalds/linux ( revision:071e31e254e0, 52998files, 2.2GB )

### Result

**ptags** is about x4 faster than universal-ctags.

| Command       | Version                         | Averaged time ( 10 times execution )  | Speed-up |
| ------------- | ------------------------------- | ------------------------------------- | -------- |
| `ctags -R`    | Universal Ctags 0.0.0(f9e6e3c1) | 23.64s                                | x1       |
| `ptags -t 16` | ptags 0.1.0                     | 5.94s                                 | x3.98    |

