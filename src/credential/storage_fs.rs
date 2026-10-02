//! Anchored Unix I/O. All names below an opened private directory are single components.
use super::CredentialError as Error;
use std::{
    ffi::{CStr, CString},
    fs::{DirBuilder, File, OpenOptions, TryLockError},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::Path,
};
pub(super) struct Directory(File);
impl Directory {
    pub fn open(root: &Path) -> Result<Self, Error> {
        match DirBuilder::new().mode(0o700).create(root) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Error::Storage),
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(|_| Error::Storage)?;
        Self::checked(file)
    }
    fn checked(file: File) -> Result<Self, Error> {
        let metadata = file.metadata().map_err(|_| Error::Storage)?;
        if !metadata.is_dir() || !owner_only(&metadata) {
            return Err(Error::Storage);
        }
        Ok(Self(file))
    }
    pub fn child(&self, name: &str, create: bool) -> Result<Option<Self>, Error> {
        self.check()?;
        let name = component(name)?;
        if create {
            let result = unsafe { libc::mkdirat(self.0.as_raw_fd(), name.as_ptr(), 0o700) };
            if result == 0 {
                self.sync()?;
            } else if std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists {
                return Err(Error::Storage);
            }
        }
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(Error::Storage)
            };
        }
        Self::checked(unsafe { File::from_raw_fd(fd) }).map(Some)
    }
    pub fn check(&self) -> Result<(), Error> {
        if owner_only(&self.0.metadata().map_err(|_| Error::Storage)?) {
            Ok(())
        } else {
            Err(Error::Storage)
        }
    }
    pub fn file(&self, name: &str, flags: i32) -> Result<File, std::io::Error> {
        self.check()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
        let name =
            component(name).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.is_file() || !owner_only(&metadata) || metadata.nlink() != 1 {
            return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        }
        Ok(file)
    }
    pub fn exists(&self, name: &str) -> Result<bool, Error> {
        let name = component(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0
        {
            Ok(true)
        } else if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
            Ok(false)
        } else {
            Err(Error::Storage)
        }
    }
    pub fn lock(&self, name: &str) -> Result<File, Error> {
        let file = self
            .file(name, libc::O_RDWR | libc::O_CREAT)
            .map_err(|_| Error::Storage)?;
        file.try_lock().map_err(|e| match e {
            TryLockError::WouldBlock => Error::Busy,
            TryLockError::Error(_) => Error::Storage,
        })?;
        Ok(file)
    }
    pub fn sync(&self) -> Result<(), Error> {
        self.0.sync_all().map_err(|_| Error::Storage)
    }
    pub fn rename(&self, from: &str, to: &str) -> Result<(), Error> {
        let from = component(from)?;
        let to = component(to)?;
        if unsafe {
            libc::renameat(
                self.0.as_raw_fd(),
                from.as_ptr(),
                self.0.as_raw_fd(),
                to.as_ptr(),
            )
        } == 0
        {
            Ok(())
        } else {
            Err(Error::Storage)
        }
    }
    pub fn remove(&self, name: &str) -> Result<(), Error> {
        let name = component(name)?;
        if unsafe { libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(Error::Storage)
        }
    }
    pub fn names(&self) -> Result<Vec<String>, Error> {
        self.check()?;
        // A fresh open file description avoids sharing directory offsets across clones.
        let dot = c".";
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                dot.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error::Storage);
        }
        let ptr = unsafe { libc::fdopendir(fd) };
        if ptr.is_null() {
            unsafe {
                libc::close(fd);
            }
            return Err(Error::Storage);
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        let stream = Stream(ptr);
        let mut names = Vec::new();
        loop {
            // errno distinguishes end-of-directory from an I/O failure.
            errno::set_errno(errno::Errno(0));
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                if errno::errno().0 != 0 {
                    return Err(Error::Storage);
                }
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_str()
                .map_err(|_| Error::Storage)?;
            if name == "." || name == ".." {
                continue;
            }
            if names.len() == 512 {
                return Err(Error::Storage);
            }
            names.push(name.into());
        }
        names.sort();
        Ok(names)
    }
}
fn component(name: &str) -> Result<CString, Error> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(Error::Storage);
    }
    CString::new(name).map_err(|_| Error::Storage)
}
fn owner_only(metadata: &std::fs::Metadata) -> bool {
    metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o077 == 0
}
