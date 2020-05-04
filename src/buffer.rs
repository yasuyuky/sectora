use std::ffi::CString;
use std::io::{Error, ErrorKind};

#[derive(Debug)]
pub struct Buffer {
    buf: *mut libc::c_char,
    offset: isize,
    buflen: libc::size_t,
}

impl Buffer {
    pub fn new(buf: *mut libc::c_char, buflen: libc::size_t) -> Self { Self { buf, offset: 0, buflen } }

    fn write(&mut self, data: *const libc::c_char, len: usize) -> Result<*mut libc::c_char, Error> {
        if self.buflen < len + self.offset as libc::size_t {
            return Err(Error::new(ErrorKind::AddrNotAvailable, "ERANGE"));
        }
        unsafe {
            let pos = self.buf.offset(self.offset);
            std::ptr::copy(data as *mut i8, pos as *mut i8, len);
            self.offset += len as isize;
            self.buflen -= len as libc::size_t;
            Ok(pos)
        }
    }

    #[allow(clippy::cast_ptr_alignment)] // NOTE: waiting for align_offset https://github.com/rust-lang/rust/issues/44488
    fn add_pointers(&mut self, ptrs: &[*mut libc::c_char]) -> Result<*mut *mut libc::c_char, Error> {
        use std::mem::size_of;
        let step = std::cmp::max(size_of::<*mut libc::c_char>() / size_of::<libc::c_char>(), 1);
        let align_offset = self.offset % step as isize;
        self.offset += align_offset; // NOTE: remove after stabilization of align_offset
        if self.buflen < (((ptrs.len() + 1) * step) as isize + self.offset) as libc::size_t {
            return Err(Error::new(ErrorKind::AddrNotAvailable, "ERANGE"));
        }
        unsafe {
            let mem = self.buf.offset(self.offset) as *mut *mut libc::c_char;
            for (i, p) in ptrs.iter().enumerate() {
                *(mem.add(i)) = *p;
                self.offset += step as isize;
                self.buflen -= step as libc::size_t;
            }
            *(mem.add(ptrs.len())) = std::ptr::null_mut::<libc::c_char>();
            self.offset += step as isize;
            self.buflen -= step as libc::size_t;
            Ok(mem)
        }
    }

    pub fn write_string(&mut self, s: &str) -> Result<*mut libc::c_char, Error> {
        let cs = CString::new(s).unwrap();
        self.write(cs.as_ptr(), s.len() + 1)
    }

    pub fn write_vecstr(&mut self, ss: &[&str]) -> Result<*mut *mut libc::c_char, Error> {
        let mut ptrs = Vec::<*mut libc::c_char>::new();
        for s in ss {
            ptrs.push(self.write_string(s)?);
        }
        self.add_pointers(&ptrs)
    }
}
