//! Error code translation between Angzarr and Linux
//!
//! Converts between Angzarr's Rust `Result<T, AngzarrError>` and
//! Linux's integer errno values.

use angzarr_ffi::KernelError;

/// Convert a Result to Linux errno format
///
/// - Ok(value) => 0
/// - Err(error) => negative errno
pub fn result_to_errno<T>(result: Result<T, KernelError>) -> i32 {
    match result {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

/// Convert errno to Result
///
/// - 0 => Ok(())
/// - negative => Err(KernelError)
/// - positive => Err (treated as error)
pub fn errno_to_result(errno: i32) -> Result<(), KernelError> {
    if errno == 0 {
        Ok(())
    } else if errno > 0 {
        // Positive values shouldn't happen but treat as EINVAL
        Err(KernelError::EINVAL)
    } else {
        // Convert negative errno back to error
        match -errno {
            1 => Err(KernelError::EPERM),
            2 => Err(KernelError::ENOENT),
            4 => Err(KernelError::EINTR),
            5 => Err(KernelError::EIO),
            11 => Err(KernelError::EAGAIN),
            12 => Err(KernelError::ENOMEM),
            13 => Err(KernelError::EACCES),
            14 => Err(KernelError::EFAULT),
            16 => Err(KernelError::EBUSY),
            17 => Err(KernelError::EEXIST),
            22 => Err(KernelError::EINVAL),
            28 => Err(KernelError::ENOSPC),
            _ => Err(KernelError::EINVAL), // Unknown error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_to_errno() {
        assert_eq!(result_to_errno(Ok::<(), _>(())), 0);
        assert_eq!(result_to_errno(Err::<(), _>(KernelError::ENOMEM)), -12);
        assert_eq!(result_to_errno(Err::<(), _>(KernelError::EINVAL)), -22);
    }

    #[test]
    fn test_errno_to_result() {
        assert!(errno_to_result(0).is_ok());
        assert_eq!(errno_to_result(-12), Err(KernelError::ENOMEM));
        assert_eq!(errno_to_result(-22), Err(KernelError::EINVAL));
    }

    #[test]
    fn test_round_trip() {
        let errors = [
            KernelError::EPERM,
            KernelError::ENOENT,
            KernelError::ENOMEM,
            KernelError::EINVAL,
        ];

        for error in &errors {
            let errno = error.to_errno();
            let result = errno_to_result(errno);
            assert_eq!(result, Err(*error));
        }
    }
}
