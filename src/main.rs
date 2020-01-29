use arglex::lex;
use arglex::Arg;
use std::env;

#[derive(Debug)]
enum ArgType {
    PrintThing(String),
}

const HELP: &str = "
testproject version 1.0.0
Usage: testproject [options]
Options:
  -h, --help                Print this help message
  --version                 Print the version number
  -p, --print-thing WHAT    Print the value of WHAT
";

fn print_help() -> ! {
    eprintln!("{}", HELP);
    std::process::exit(0);
}

fn print_version() -> ! {
    println!("testproject version 1.0.0");
    std::process::exit(0);
}

fn needs_arg(arg: &str) -> ! {
    eprintln!("{} needs an argument. See --help for more details.", arg);
    std::process::exit(1);
}

fn unknown_arg(which: &Arg) -> ! {
    eprintln!(
        "error: an unknown {} argument was passed: {}\nSee --help for a list of valid arguments.",
        match which {
            Arg::Positional(_) => "positional",
            Arg::Short(_) => "short",
            Arg::Long(_) => "long",
        },
        which
    );
    std::process::exit(1);
}

fn main() {
    let args = lex(env::args().skip(1).collect());
    let mut args = args.iter();
    let mut passed_args: Vec<ArgType> = vec![];
    while let Some(arg) = args.next() {
        let arg_type = match arg {
            Arg::Positional(positional) => match positional.as_str() {
                "--" => continue,
                _ => unknown_arg(arg),
            },
            Arg::Long(long) => match long.as_str() {
                "help" => print_help(),
                "version" => print_version(),
                "print-thing" => {
                    if let Arg::Positional(what) = args.next().unwrap_or_else(|| needs_arg(long)) {
                        ArgType::PrintThing(what.clone())
                    } else {
                        needs_arg(long)
                    }
                }
                _ => unknown_arg(arg),
            },
            Arg::Short(short) => match short.as_str() {
                "h" => print_help(),
                "p" => {
                    if let Arg::Positional(what) = args.next().unwrap_or_else(|| needs_arg(short)) {
                        ArgType::PrintThing(what.clone())
                    } else {
                        needs_arg(short)
                    }
                }
                _ => unknown_arg(arg),
            },
        };
        passed_args.push(arg_type);
    }
    for arg in passed_args {
        match arg {
            ArgType::PrintThing(what) => println!("{}", what),
        }
    }
}
