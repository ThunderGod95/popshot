use rustix::fs::{self, Mode, OFlags};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id)
}

fn directory_at(base: &Path) -> io::Result<File> {
    std::fs::create_dir_all(base)?;

    let parent = File::open(base)?;

    let name = crate::activation::APP_ID;

    match fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(error.into()),
    }

    // Hold the directory FD so path replacement cannot redirect screenshot access.
    let directory = File::from(fs::openat(
        &parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);

    if fs::fstat(&directory)?.st_uid != rustix::process::geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Cache directory is owned by another user",
        ));
    }

    fs::fchmod(&directory, Mode::from_raw_mode(0o700))?;

    Ok(directory)
}

fn directory() -> io::Result<File> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".cache"))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "No absolute XDG_CACHE_HOME or HOME",
            )
        })?;

    directory_at(&base)
}

fn write(directory: &File, png: &[u8]) -> io::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();

    let name = format!("{id}.png");

    let mut file = File::from(fs::openat(
        directory,
        &name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?);

    if let Err(error) = file.write_all(png).and_then(|()| file.sync_all()) {
        let _ = fs::unlinkat(directory, &name, fs::AtFlags::empty());
        return Err(error);
    }

    Ok(id)
}

fn read(directory: &File, id: &str) -> io::Result<Vec<u8>> {
    if !valid_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid capture ID",
        ));
    }

    let mut file = File::from(fs::openat(
        directory,
        format!("{id}.png"),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?);

    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;

    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Capture is not a private regular file",
        ));
    }

    let mut png = Vec::new();

    file.read_to_end(&mut png)?;

    Ok(png)
}

pub fn store(png: &[u8]) -> Result<String, String> {
    directory()
        .and_then(|directory| write(&directory, png))
        .map_err(|e| format!("Could not cache screenshot: {e}"))
}

pub fn load(id: &str) -> Result<crate::capture::CapturedImage, String> {
    let png = directory()
        .and_then(|directory| read(&directory, id))
        .map_err(|e| format!("Cached screenshot is unavailable: {e}"))?;
    crate::capture::decode(png)
}

pub fn purge() {
    if let Err(error) = directory().and_then(|directory| purge_at(&directory, SystemTime::now())) {
        eprintln!("Could not clean screenshot cache: {error}");
    }
}

fn purge_at(directory: &File, now: SystemTime) -> io::Result<()> {
    let now = now.duration_since(UNIX_EPOCH).map_err(io::Error::other)?;
    let cutoff = (
        now.as_secs() as i64 - 24 * 60 * 60,
        now.subsec_nanos() as u64,
    );
    let mut screenshots = Vec::new();

    for entry in fs::Dir::read_from(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if !name
            .to_str()
            .ok()
            .and_then(|name| name.strip_suffix(".png"))
            .is_some_and(valid_id)
        {
            continue;
        }

        let metadata = match fs::statat(directory, name, fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => metadata,
            Err(rustix::io::Errno::NOENT) => continue,
            Err(error) => {
                eprintln!("Could not inspect cached screenshot {name:?}: {error}");
                continue;
            }
        };
        if fs::FileType::from_raw_mode(metadata.st_mode) == fs::FileType::RegularFile {
            screenshots.push(((metadata.st_mtime, metadata.st_mtime_nsec), name.to_owned()));
        }
    }

    // Newest first; filenames break ties for files with identical timestamps.
    screenshots.sort_unstable_by(|a, b| b.cmp(a));
    for (index, (modified, name)) in screenshots.into_iter().enumerate() {
        if modified < cutoff || index >= 20 {
            match fs::unlinkat(directory, &name, fs::AtFlags::empty()) {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(error) => {
                    eprintln!("Could not remove cached screenshot {name:?}: {error}");
                }
            }
        }
    }
    Ok(())
}