#[macro_use]
extern crate clap;

mod c_bindings;
use c_bindings::*;
use chrono::offset::TimeZone;
use chrono::DateTime;
use chrono::Datelike;
use chrono::Local;
use chrono::NaiveDate;
use chrono::Timelike;
use clap::App;
use libc::timespec;
use std::env::current_exe;
use std::ffi::CString;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;
use std::fs::File;

struct TouchFlags {
    change_only_access_time: bool,
    change_only_modification_time: bool,
    affect_symlinks: bool,
    no_creating_files: bool,
    accessed_time: DateTime<Local>,
    modified_time: DateTime<Local>,
}

fn parse_timestamp(timestamp: &str) -> Result<DateTime<Local>, &'static str> {
    let chars: Vec<char> = timestamp.chars().collect();
    let has_seconds = chars.contains(&'.');
    let expected_len = 8 + if has_seconds { 3 } else { 0 };
    if timestamp.len() < expected_len {
        return Err("Timestamp is too short");
    }
    let has_century = chars.len() == expected_len + 4;
    let has_year = has_century || chars.len() == expected_len + 2;
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
    let raw_month = &rest[0..2];
    let raw_day = &rest[2..4];
    let raw_hour = &rest[4..6];
    let raw_minute = &rest[6..8];
    let raw_seconds = if has_seconds { &rest[9..] } else { "" };
    let now = Local::now();
    let century: i32 = if has_century {
        if let Ok(century) = raw_century.parse::<i32>() {
            century * 100
        } else {
            return Err("Invalid century");
        }
    } else {
        now.year() / 100 * 100
    };
    let year: i32 = if has_year {
        if let Ok(year) = raw_year.parse() {
            year
        } else {
            return Err("Invalid year");
        }
    } else {
        now.year() % 100
    };
    let month: u32 = if let Ok(month) = raw_month.parse() {
        month
    } else {
        return Err("Invalid month");
    };
    let day: u32 = if let Ok(day) = raw_day.parse() {
        day
    } else {
        return Err("Invalid day");
    };
    let hour: u32 = if let Ok(hour) = raw_hour.parse() {
        hour
    } else {
        return Err("Invalid hour");
    };
    let minute: u32 = if let Ok(minute) = raw_minute.parse() {
        minute
    } else {
        return Err("Invalid minute");
    };
    let second: u32 = if has_seconds {
        if let Ok(second) = raw_seconds.parse::<u32>() {
            second
        } else {
            return Err("Invalid second");
        }
    } else {
        now.second()
    };
    if let Some(date) = NaiveDate::from_ymd_opt(century + year, month, day)
        .and_then(|d| d.and_hms_opt(hour, minute, second))
    {
        Ok(Local.from_local_datetime(&date).unwrap())
    } else {
        Err("Invalid date")
    }
}

fn main() -> Result<(), String> {
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
                _ => return Err(format!("Invalid argument to --time: {}", time)),
            }
        } else {
            (
                matches.is_present("access"),
                matches.is_present("modification"),
            )
        };
    if change_only_access_time && change_only_modification_time {
        return Err("-a and -m are mutually exclusive".to_owned());
    }

    let no_creating_files = matches.is_present("nocreate");
    let affect_symlinks = matches.is_present("nodereference");
    let (accessed_time, modified_time) = {
        if let Some(date) = matches.value_of("date") {
            match DateTime::parse_from_rfc3339(date) {
                Ok(time) => {
                    let local_time = time.with_timezone(&Local);
                    (local_time, local_time)
                }
                Err(e) => {
                    return Err(format!("Error parsing {} as an RFC 3339 date: {}", date, e));
                }
            }
        } else if let Some(timestamp) = matches.value_of("timestamp") {
            match parse_timestamp(timestamp) {
                Ok(time) => (time, time),
                Err(e) => {
                    return Err(format!("Error parsing {} as a timestamp: {}", timestamp, e));
                }
            }
        } else if let Some(reference) = matches.value_of("reference") {
            let path = PathBuf::from(reference);
            if path.exists() {
                if let Ok(metadata) = path.as_path().metadata() {
                    (
                        DateTime::<Local>::from(
                            metadata.accessed().unwrap_or_else(|_| SystemTime::now()),
                        ),
                        DateTime::<Local>::from(
                            metadata.modified().unwrap_or_else(|_| SystemTime::now()),
                        ),
                    )
                } else {
                    return Err(format!("Cannot stat referenced file {}", reference));
                }
            } else {
                return Err(format!("Referenced file {} does not exist", reference));
            }
        } else {
            let now = Local::now();
            (now, now)
        }
    };
    let files: Vec<&str> = matches.values_of("FILE").unwrap().collect();
    let flags = TouchFlags {
        change_only_access_time,
        change_only_modification_time,
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

fn touch(file_name: &str, flags: &TouchFlags) -> Result<(), String> {
    if !PathBuf::from(file_name).exists() {
        if flags.no_creating_files {
            println!(
                "Skipping {} as --no-create was passed and it does not already exist",
                file_name
            );
            return Ok(());
        } else if let Err(e) = File::create(PathBuf::from(file_name)) {
            return Err(format!("{}", e));
        }
    }
    let atime = timespec {
        tv_sec: flags.accessed_time.timestamp(),
        tv_nsec: if !flags.change_only_modification_time {
            flags.accessed_time.timestamp_subsec_nanos() as i64
        } else {
            unsafe { C_UTIME_OMIT as i64 }
        },
    };
    let mtime = timespec {
        tv_sec: flags.modified_time.timestamp(),
        tv_nsec: if !flags.change_only_access_time {
            flags.modified_time.timestamp_subsec_nanos() as i64
        } else {
            unsafe { C_UTIME_OMIT as i64 }
        },
    };
    let c_file_name = CString::new(file_name).unwrap().into_bytes_with_nul();
    let flag = if flags.affect_symlinks {
        0
    } else {
        unsafe { C_AT_SYMLINK_NOFOLLOW as i32 }
    };
    let res = unsafe {
        utimensat(
            C_AT_FDCWD,
            c_file_name.as_ptr(),
            [atime, mtime].as_ptr(),
            flag,
        )
    };
    if res != 0 {
        let error = io::Error::last_os_error();
        return Err(format!(
            "Could not set time(s) for {}: {}",
            file_name, error
        ));
    }
    Ok(())
}
