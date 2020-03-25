#[macro_use]
extern crate clap;
use crate::lib::GitRepository;
use clap::App;

mod lib;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    if let Some(matches) = matches.subcommand_matches("init") {
        let path = matches.value_of("path").unwrap();
        GitRepository::repo_create(path).unwrap();
    }
}
