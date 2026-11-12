use super::*;

#[test]
fn native_error_classes_preserve_unknown_versus_successful_zero() {
    assert_eq!(reading(Ok(0u64)), Reading::Available { value: 0 });
    for code in [libc::EPERM, libc::EACCES] {
        assert_eq!(
            reading::<u64>(Err(io::Error::from_raw_os_error(code))),
            Reading::Denied {}
        );
    }
    for code in [libc::ENOENT, libc::ENOSYS, libc::ENOTSUP] {
        assert_eq!(
            reading::<u64>(Err(io::Error::from_raw_os_error(code))),
            Reading::Unsupported {}
        );
    }
    assert!(
        matches!(reading::<u64>(Err(io::Error::other("private native text"))), Reading::Failed { diagnostic } if diagnostic.as_str() == "native_measurement_failed")
    );
    assert!(mach_result(libc::KERN_SUCCESS).is_ok());
    assert_eq!(
        mach_result(libc::KERN_PROTECTION_FAILURE)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(mach_result(libc::KERN_INVALID_ARGUMENT).is_err());
}

#[test]
fn available_ram_uses_free_plus_inactive_without_speculative_or_compressed_overlap() {
    assert_eq!(available_bytes(3, 7, 16_384).unwrap(), 163_840);
    assert_eq!(available_bytes(0, 0, 4096).unwrap(), 0);
    assert!(available_bytes(1, 2, 0).is_err());
    assert!(available_bytes(u32::MAX, u32::MAX, u64::MAX).is_err());
}

#[test]
fn pressure_is_a_native_discrete_flag_and_paths_are_bounded_native_text() {
    assert_eq!(pressure_level(1).unwrap(), MemoryPressure::Normal);
    assert_eq!(pressure_level(2).unwrap(), MemoryPressure::Warning);
    assert_eq!(pressure_level(4).unwrap(), MemoryPressure::Critical);
    for invalid in [0, 3, -1, 5, i32::MAX] {
        assert!(pressure_level(invalid).is_err());
    }
    assert_eq!(native_text(&[47, 0, 1]).unwrap(), "/");
    assert!(native_text(&[47, 47]).is_err());
    assert!(native_text(&[-1, 0]).is_err());
}
