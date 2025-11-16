//! Linux-compatible type definitions
//!
//! Provides Linux kernel type aliases that map to Angzarr types.

use angzarr_core::{Pid, Uid, Gid};
use angzarr_ffi::{c_int, c_long, c_ulong};

// Linux kernel type aliases
pub type pid_t = Pid;
pub type uid_t = Uid;
pub type gid_t = Gid;

// Standard C types (already defined in libc, but provided for completeness)
pub type int = c_int;
pub type long = c_long;
pub type unsigned_long = c_ulong;

/// Get PID value
#[no_mangle]
pub extern "C" fn pid_vnr(pid: Pid) -> c_int {
    pid.0
}

/// Get UID value
#[no_mangle]
pub extern "C" fn uid_value(uid: Uid) -> c_int {
    uid.0 as c_int
}

/// Get GID value
#[no_mangle]
pub extern "C" fn gid_value(gid: Gid) -> c_int {
    gid.0 as c_int
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_vnr() {
        let pid = Pid(1234);
        assert_eq!(pid_vnr(pid), 1234);
    }

    #[test]
    fn test_uid_value() {
        let uid = Uid(1000);
        assert_eq!(uid_value(uid), 1000);
    }
}
