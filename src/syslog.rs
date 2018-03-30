#[macro_export]
macro_rules! syslog {
    ($level:path, $msg:expr) => (
        unsafe {
            libc::openlog("sectora".as_ptr() as *const libc::c_char, libc::LOG_PID, libc::LOG_AUTH);
            libc::syslog($level, $msg.as_ptr() as *const libc::c_char);
            libc::closelog();
        }
    )
}
