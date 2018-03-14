#[macro_use]
extern crate bencher;
extern crate ptagslib;
extern crate structopt;

use bencher::Bencher;
use ptagslib::bin::{Opt, run_opt};
use structopt::StructOpt;

fn bench_self(bench: &mut Bencher) {
    bench.iter(|| {
        let args = vec!["ptags"];
        let opt = Opt::from_iter(args.iter());
        let _ = run_opt(&opt);
    })
}

benchmark_group!(benches, bench_self);
benchmark_main!(benches);
