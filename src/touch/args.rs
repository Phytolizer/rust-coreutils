use arglex::lex;
use arglex::Arg;
use crate::TouchError;

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::process::exit;
use std::slice::Iter;

const HELP: &str = "
touch version 1.0.0
By Kyle Coffey <kylecoffey1999@gmail.com>

Update the access and modification times of each FILE to the current time.

A FILE argument that does not exist is created empty, unless -c or -h is supplied.

A FILE argument string of - is handled specially and causes touch to change the times of the file associated with standard output.

Usage: touch [options] <FILE> ...

Options:
  -a                        Change only the access time
  -c, --no-create           Do not create any files
  -d, --date <STRING>       Parse STRING and use it instead of the current time
  -h, --no-dereference      Affect each symbolic link instead of any referenced file
  -m                        Change only the modification time
  -r, --reference <FILE>    Use the times of FILE instead of the current time
  -t <STAMP>                Use [[CC]YY]MMDDhhmm[.ss] instead of the current time
  --time <WORD>             Change the specified time:
                              if WORD is access, atime, or use: equivalent to -a
                              if WORD is modify or mtime: equivalent to -m
  --version                 Output version information and exit
  --help                    Display this help and exit
";

fn print_help() -> ! {
    eprintln!("{}", HELP);
    exit(0);
}

fn print_version() -> ! {
    eprintln!("touch version 1.0.0");
    exit(0);
}

pub struct Args {
    pub access: bool,
    pub no_create: bool,
    pub date: Option<String>,
    pub no_dereference: bool,
    pub modification: bool,
    pub reference: Option<String>,
    pub timestamp: Option<String>,
    pub time: Option<String>,
    pub files: Vec<String>,
}

impl Args {
    fn new() -> Self {
        Args {
            access: false,
            no_create: false,
            date: None,
            no_dereference: false,
            modification: false,
            reference: None,
            timestamp: None,
            time: None,
            files: vec![],
        }
    }
}

pub struct ArgError {
    why: String,
}

impl Into<TouchError> for ArgError {
    fn into(self) -> TouchError {
        TouchError::from(format!("{:?}", self))
    }
}

impl Debug for ArgError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.why)
    }
}

impl<T> From<T> for ArgError
where
    T: ToString,
{
    fn from(err: T) -> ArgError {
        ArgError {
            why: err.to_string(),
        }
    }
}

fn get_arg_to(args: &mut Iter<Arg>) -> Result<String, ArgError> {
    let arg = args.next();
    let error_message = "An argument must be supplied".to_owned();
    if let Some(arg) = arg {
        if let Arg::Positional(value) = arg {
            return Ok(value.clone());
        }
    }
    Err(error_message.into())
}

fn unknown_argument(arg: &Arg) -> ArgError {
    format!("unknown argument {}", arg).into()
}

pub fn parse(args: Vec<String>) -> Result<Args, ArgError> {
    let args = lex(args);

    let mut args = args.iter();
    let mut arg_struct = Args::new();
    while let Some(arg) = args.next() {
        match arg {
            Arg::Positional(positional) => match positional.as_str() {
                "--" => continue,
                _ => arg_struct.files.push(positional.clone()),
            },
            Arg::Short(short) => match short.as_str() {
                "a" => arg_struct.access = true,
                "c" => arg_struct.no_create = true,
                "d" => arg_struct.date = Some(get_arg_to(&mut args)?),
                "h" => arg_struct.no_dereference = true,
                "m" => arg_struct.modification = true,
                "r" => arg_struct.reference = Some(get_arg_to(&mut args)?),
                "t" => arg_struct.timestamp = Some(get_arg_to(&mut args)?),
                _ => return Err(unknown_argument(arg)),
            },
            Arg::Long(long) => match long.as_str() {
                "no-create" => arg_struct.no_create = true,
                "date" => arg_struct.date = Some(get_arg_to(&mut args)?),
                "no-dereference" => arg_struct.no_dereference = true,
                "reference" => arg_struct.reference = Some(get_arg_to(&mut args)?),
                "time" => arg_struct.time = Some(get_arg_to(&mut args)?),
                "version" => print_version(),
                "help" => print_help(),
                _ => {}
            },
        };
    }
    Ok(arg_struct)
}
