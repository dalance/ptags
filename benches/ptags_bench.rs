#[macro_use]
extern crate bencher;
extern crate ptagslib;
extern crate structopt;

use bencher::Bencher;
use ptagslib::bin::{Opt, run_opt};
use structopt::StructOpt;

fn bench_default(bench: &mut Bencher) {
    bench.iter(|| {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let _ = run_opt(&opt);
    })
}

fn bench_unsorted(bench: &mut Bencher) {
    bench.iter(|| {
        let args = vec!["ptags", "--unsorted"];
        let opt = Opt::from_iter(args.iter());
        let _ = run_opt(&opt);
    })
}

benchmark_group!(benches, bench_default, bench_unsorted);
benchmark_main!(benches);
