use ptagslib::bin::run;

// ---------------------------------------------------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------------------------------------------------

fn main() {
    match run() {
        Err(x) => {
            println!("{}", x);
            for x in x.chain() {
                println!("{}", x);
            }
        }
        _ => (),
    }
}
