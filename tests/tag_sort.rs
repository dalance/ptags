use ptagslib::bin::{run_opt, Opt};
use std::fs;
use structopt::StructOpt;

fn generate_tags(unsorted: bool) -> String {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(
        fixture.path().join("sample.c"),
        "int zeta(void) { return 1; }\nint alpha(void) { return 2; }\n",
    )
    .unwrap();
    let list = fixture.path().join("files.txt");
    fs::write(&list, "sample.c\n").unwrap();
    let output = fixture.path().join("tags");
    let mut args = vec![
        "ptags".into(),
        "-t".into(),
        "1".into(),
        "-L".into(),
        list.into_os_string(),
        "-f".into(),
        output.clone().into_os_string(),
    ];
    if unsorted {
        args.push("--unsorted".into());
    }
    args.push(fixture.path().as_os_str().to_owned());
    run_opt(&Opt::from_iter(args)).unwrap();
    fs::read_to_string(output).unwrap()
}

#[test]
fn unsorted_header_matches_tag_order() {
    let tags = generate_tags(true);
    assert!(tags
        .lines()
        .any(|line| line.starts_with("!_TAG_FILE_SORTED\t0\t")));
    let names: Vec<_> = tags
        .lines()
        .filter(|line| !line.starts_with('!'))
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    assert_eq!(names, ["zeta", "alpha"]);
}

#[test]
fn default_header_matches_tag_order() {
    let tags = generate_tags(false);
    assert!(tags
        .lines()
        .any(|line| line.starts_with("!_TAG_FILE_SORTED\t1\t")));
    let names: Vec<_> = tags
        .lines()
        .filter(|line| !line.starts_with('!'))
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    assert_eq!(names, ["alpha", "zeta"]);
}
