use std::{
    env,
    fs::{File, OpenOptions, TryLockError},
    io,
    path::PathBuf,
};

pub struct InstanceGuard {
    _file: File,
}

impl InstanceGuard {
    pub fn acquire(name: &str) -> io::Result<Option<Self>> {
        let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "XDG_RUNTIME_DIR is not set",
                )
            })?;

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(runtime_dir.join(format!("{name}.lock")))?;

        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}
