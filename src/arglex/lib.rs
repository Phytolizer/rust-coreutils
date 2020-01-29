use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
pub enum Arg {
    Positional(String),
    Short(String),
    Long(String),
}

impl Display for Arg {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Arg::Positional(arg) => write!(f, "{}", arg),
            Arg::Short(arg) => write!(f, "-{}", arg),
            Arg::Long(arg) => write!(f, "--{}", arg),
        }
    }
}

fn arg_of(raw_arg: String, delimited: &mut bool) -> (Arg, Option<String>) {
    if *delimited {
        (Arg::Positional(raw_arg), None)
    } else if raw_arg.starts_with("--") {
        if raw_arg.len() == 2 {
            *delimited = true;
            (Arg::Positional(raw_arg), None)
        } else if let Some(i) = raw_arg.find('=') {
            (
                Arg::Long(raw_arg[2..i].to_string()),
                Some(raw_arg[i + 1..].to_string()),
            )
        } else {
            (Arg::Long(raw_arg[2..].to_string()), None)
        }
    } else if raw_arg.starts_with('-') {
        if raw_arg.len() == 1 {
            (Arg::Positional(raw_arg), None)
        } else {
            (
                Arg::Short(raw_arg[1..2].to_string()),
                if raw_arg.len() > 2 {
                    Some(raw_arg[2..].to_string())
                } else {
                    None
                },
            )
        }
    } else {
        (Arg::Positional(raw_arg), None)
    }
}

pub fn lex(raw_args: Vec<String>) -> Vec<Arg> {
    let mut args: Vec<Arg> = vec![];
    let mut delimited = false;
    for raw_arg in raw_args {
        let (arg, rest) = arg_of(raw_arg, &mut delimited);
        args.push(arg);
        if let Some(rest) = rest {
            args.push(Arg::Positional(rest));
        }
    }

    args
}
