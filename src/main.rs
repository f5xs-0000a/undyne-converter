mod criu;

use std::process::Command;
use std::io::stdin;
use std::io::BufRead as _;
use std::process::Stdio;
use std::process::ChildStdout;
use clap::Parser;
//use std::os::unix::process::CommandExt as _;

#[derive(Parser)]
#[command(author = "F5XS")]
enum Kind {
    Run(RunApp),
    Add(AddApp),
}

fn main() {
    match Kind::parse() {
        
    }
}
