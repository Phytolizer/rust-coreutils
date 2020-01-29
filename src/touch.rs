#[macro_use]
extern crate clap;

mod c_bindings;
use c_bindings::AT_FDCWD;
use c_bindings::AT_SYMLINK_NOFOLLOW;
use c_bindings::UTIME_OMIT;
use chrono::offset::TimeZone;
use chrono::DateTime;
use chrono::Datelike;
use chrono::Local;
use chrono::NaiveDate;
use clap::App;
use libc::timespec;
use std::env::current_exe;
use std::ffi::CString;
use std::fmt;
use std::fmt::Debug;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;
use syscall::syscall;

struct TouchFlags {
    change_access_time: bool,
    change_modification_time: bool,
    affect_symlinks: bool,
    no_creating_files: bool,
    accessed_time: DateTime<Local>,
    modified_time: DateTime<Local>,
}

struct TouchError {
    message: String,
}

impl Debug for TouchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "touch: {}", self.message)?;
        Ok(())
    }
}

impl<T> From<T> for TouchError
where
    T: ToString,
{
    fn from(error: T) -> Self {
        TouchError {
            message: error.to_string(),
        }
    }
}

fn parse_timestamp(timestamp: &str) -> Result<DateTime<Local>, TouchError> {
    let chars: Vec<char> = timestamp.chars().collect();
    // the "has_" flags are checking for optional parts of the timestamp string.
    // Not much validation is done here until we try to parse integers.
    let has_seconds = chars.contains(&'.');
    let expected_len = 8 + if has_seconds { 3 } else { 0 };
    if timestamp.len() < expected_len {
        return Err("timestamp is too short".into());
    }
    let has_century = chars.len() == expected_len + 4;
    let has_year = has_century || chars.len() == expected_len + 2;
    // Take slices for the significant parts of the timestamp to clean up later code.
    // "Shift" the input by using a `rest` slice.
    let (raw_century, rest) = if has_century {
        (&timestamp[0..2], &timestamp[2..])
    } else {
        ("", timestamp)
    };
    let (raw_year, rest) = if has_year {
        (&rest[0..2], &rest[2..])
    } else {
        ("", rest)
    };
    // No more shifting here, it's unnecessary
    let raw_month = &rest[0..2];
    let raw_day = &rest[2..4];
    let raw_hours = &rest[4..6];
    let raw_minutes = &rest[6..8];
    let raw_seconds = if has_seconds { &rest[9..] } else { "" };

    // Missing fields will be substituted with the current date
    let now = Local::today();

    // Try and parse the fields now
    let century: i32 = if has_century {
        raw_century.parse::<i32>().map_err(|_| "invalid century")? * 100
    } else {
        now.year() / 100 * 100
    };
    let year: i32 = if has_year {
        raw_year.parse().map_err(|_| "invalid year")?
    } else {
        now.year() % 100
    };

    let month: u32 = raw_month.parse().map_err(|_| "invalid month")?;
    let day: u32 = raw_day.parse().map_err(|_| "invalid day")?;
    let hours: u32 = raw_hours.parse().map_err(|_| "invalid hour")?;
    let minutes: u32 = raw_minutes.parse().map_err(|_| "invalid minute")?;
    let seconds: u32 = if has_seconds {
        raw_seconds.parse::<u32>().map_err(|_| "invalid second")?
    } else {
        0
    };

    // Done! Construct a DateTime (and also check none of the numbers were OOB)
    if let Some(date) = NaiveDate::from_ymd_opt(century + year, month, day)
        .and_then(|d| d.and_hms_opt(hours, minutes, seconds))
    {
        Ok(Local.from_local_datetime(&date).unwrap())
    } else {
        Err("invalid date".into())
    }
}

fn main() -> Result<(), TouchError> {
    let yaml = load_yaml!("touch.yaml");
    let matches = App::from(yaml).get_matches();
    if matches.is_present("version") {
        println!(
            "{} version 1.0.0",
            current_exe()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
        );
        return Ok(());
    }
    let (change_only_access_time, change_only_modification_time) =
        if let Some(time) = matches.value_of("time") {
            match time {
                "access" | "atime" | "use" => (true, false),
                "modify" | "mtime" => (false, true),
                _ => return Err(format!("invalid argument to --time: {}", time).into()),
            }
        } else {
            (
                matches.is_present("access"),
                matches.is_present("modification"),
            )
        };
    if change_only_access_time && change_only_modification_time {
        return Err("-a and -m are mutually exclusive".into());
    }
    let change_access_time = !change_only_modification_time || change_only_access_time;
    let change_modification_time = !change_only_access_time || change_only_modification_time;

    let no_creating_files = matches.is_present("nocreate");
    let affect_symlinks = matches.is_present("nodereference");
    let (accessed_time, modified_time) = {
        if let Some(date) = matches.value_of("date") {
            let time = DateTime::parse_from_rfc3339(date)
                .map_err(|e| format!("error parsing {} as an RFC 3339 date: {}", date, e))?;
            let local_time = time.with_timezone(&Local);
            (local_time, local_time)
        } else if let Some(timestamp) = matches.value_of("timestamp") {
            let time = parse_timestamp(timestamp)
                .map_err(|e| format!("error parsing {} as a timestamp: {:?}", timestamp, e))?;
            (time, time)
        } else if let Some(reference) = matches.value_of("reference") {
            let reference_path = PathBuf::from(reference);
            if !reference_path.exists() {
                return Err(format!("referenced file {} does not exist", reference).into());
            }
            let metadata = reference_path
                .as_path()
                .metadata()
                .map_err(|_| format!("cannot stat referenced file {}", reference))?;
            (
                metadata
                    .accessed()
                    .unwrap_or_else(|_| SystemTime::now())
                    .into(),
                metadata
                    .modified()
                    .unwrap_or_else(|_| SystemTime::now())
                    .into(),
            )
        } else {
            let now = Local::now();
            (now, now)
        }
    };
    if !matches.is_present("FILE") {
        return Err("must specify at least one file".into());
    }
    let files: Vec<&str> = matches.values_of("FILE").unwrap().collect();
    let flags = TouchFlags {
        change_access_time,
        change_modification_time,
        affect_symlinks,
        no_creating_files,
        accessed_time,
        modified_time,
    };
    for file in files {
        touch(file, &flags)?;
    }
    Ok(())
}

fn touch(file_name: &str, flags: &TouchFlags) -> Result<(), TouchError> {
    if !PathBuf::from(file_name).exists() {
        if flags.no_creating_files {
            println!(
                "Skipping {} as --no-create was passed and it does not already exist",
                file_name
            );
            return Ok(());
        } else if let Err(e) = File::create(PathBuf::from(file_name)) {
            return Err(e.into());
        }
    }
    let atime = timespec {
        tv_sec: flags.accessed_time.timestamp(),
        tv_nsec: if !flags.change_modification_time || flags.change_access_time {
            flags.accessed_time.timestamp_subsec_nanos() as i64
        } else {
            UTIME_OMIT as i64
        },
    };
    let mtime = timespec {
        tv_sec: flags.modified_time.timestamp(),
        tv_nsec: if !flags.change_access_time || flags.change_modification_time {
            flags.modified_time.timestamp_subsec_nanos() as i64
        } else {
            UTIME_OMIT as i64
        },
    };
    let c_file_name = CString::new(file_name).unwrap().into_bytes_with_nul();
    let flag = if flags.affect_symlinks {
        0
    } else {
        AT_SYMLINK_NOFOLLOW
    };
    let ret = unsafe {
        syscall!(
            UTIMENSAT,
            AT_FDCWD,
            c_file_name.as_ptr(),
            [atime, mtime].as_ptr(),
            flag
        )
    };
    if ret != 0 {
        let error = io::Error::last_os_error();
        return Err(format!("could not set time(s) for {}: {}", file_name, error).into());
    }
    Ok(())
}
