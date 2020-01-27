extern "C" {
    pub fn utimensat(fd: i32, path: *const u8, times: *const libc::timespec, flag: i32) -> i32;
}
#[link(name = "cconstants", kind = "static")]
extern "C" {
    pub static C_UTIME_OMIT: libc::c_int;
    pub static C_AT_SYMLINK_NOFOLLOW: libc::c_int;
    pub static C_AT_FDCWD: libc::c_int;
}
