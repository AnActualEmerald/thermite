use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{core::utils::TempDir, error::ThermiteError};

use zip::ZipArchive;

use tracing::{debug, trace, warn};

const CHUNK_SIZE: usize = 1024;

/// Download a file and update a progress bar
/// # Params
/// * `output` - Writer to write the data to
/// * `url` - URL to download from
/// * `cb` - Callback to call with every chunk read. Params are |`delta_bytes`: u64, `current_bytes`: u64, `total_size`: u64|
///
/// # Returns
/// * total bytes downloaded & written
///
/// # Errors
/// * IO Errors
pub fn download_with_progress<F>(
    mut output: impl Write,
    url: impl AsRef<str>,
    cb: F,
) -> Result<u64, ThermiteError>
where
    F: Fn(u64, u64, u64),
{
    //send the request
    let res = ureq::get(url.as_ref()).call()?;

    let file_size = res
        .header("Content-Length")
        .unwrap_or_else(|| {
            warn!("Response missing 'Content-Length' header");
            "0"
        })
        .parse::<u64>()?;
    debug!("Downloading file of size: {}", file_size);

    //start download in chunks
    let mut downloaded: u64 = 0;
    let mut buffer = [0; CHUNK_SIZE];
    let mut body = res.into_reader();
    debug!("Starting download from {}", url.as_ref());

    while let Ok(n) = body.read(&mut buffer) {
        output.write_all(&buffer[0..n])?;
        downloaded += n as u64;

        cb(n as u64, downloaded, file_size);

        if n == 0 {
            break;
        }
    }

    Ok(downloaded)
}

/// Wrapper for calling `download_with_progress` without a progress bar
/// # Params
/// * `output` - Writer to write the data to
/// * `url` - Url to download from
///
/// # Returns
/// * total bytes downloaded & written
///
/// # Errors
/// * IO Errors
pub fn download(output: impl Write, url: impl AsRef<str>) -> Result<u64, ThermiteError> {
    download_with_progress(output, url, |_, _, _| {})
}

#[deprecated]
pub fn uninstall(mods: &[impl AsRef<Path>]) -> Result<(), ThermiteError> {
    for p in mods {
        if fs::remove_dir_all(p).is_err() {
            //try removing a file too, just in case
            debug!("Removing dir failed, attempting to remove file...");
            fs::remove_file(p)?;
        }
    }
    Ok(())
}

/// Install a mod to a directory
/// # Params
/// * `zip_file` - compressed mod file
/// * `target_dir` - directory to install to
/// * `extract_dir` - directory to extract to before installing. Defaults to a temp directory in `target_dir`
/// * `sanity_check` - function that will be called before performing the installation. The operation will fail with `ThermiteError::SanityError` if this returns `false`
///     - takes `File` of the zip file
///     - returns `bool`
///
/// `target_dir` will be treated as the root of the `mods` directory in the mod file
////// # Errors
/// * IO Errors
/// * Misformatted mods (typically missing the `mods` directory)
///
/// # Panics
/// This function will panic if it is unable to get the current system time
pub fn install_with_sanity<T, F>(
    author: impl AsRef<str>,
    zip_file: T,
    target_dir: impl AsRef<Path>,
    extract_dir: Option<&Path>,
    sanity_check: F,
) -> Result<Vec<PathBuf>, ThermiteError>
where
    T: Read + Seek,
    F: FnOnce(&T) -> bool,
{
    let target_dir = target_dir.as_ref();
    if !sanity_check(&zip_file) {
        return Err(ThermiteError::SanityError);
    }
    debug!("Starting mod insall");
    let mods_dir = target_dir.canonicalize()?;
    //Extract mod to a temp directory so that we can easily see any sub-mods
    //This wouldn't be needed if the ZipArchive recreated directories, but oh well
    let temp_dir = if let Some(p) = extract_dir {
        p.to_path_buf()
    } else {
        mods_dir.join(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Unable to get system time")
                .as_secs()
                .to_string(),
        )
    };

    // TempDir ensures the directory is removed when it goes out of scope
    let temp_dir = TempDir::create(temp_dir)?;
    {
        let mut archive = ZipArchive::new(zip_file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.enclosed_name().is_none() {
                trace!("Skip missing enclosed name '{}'", file.name());
                continue;
            }
            let out = temp_dir.join(file.enclosed_name().unwrap());

            if file.enclosed_name().unwrap().starts_with(".") {
                debug!("Skipping hidden file {}", out.display());
                continue;
            }

            debug!("Extracting file to {}", out.display());
            if (*file.name()).ends_with('/') {
                trace!("Creating dir path in temp dir");
                fs::create_dir_all(&out)?;
                continue;
            } else if let Some(p) = out.parent() {
                trace!("Creating dir at {}", p.display());
                fs::create_dir_all(p)?;
            }
            trace!("Open file {} for writing", out.display());
            let mut outfile = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&out)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    let mut mods = vec![];
    // find all the submods that need to be installed
    if let Ok(entries) = temp_dir.read_dir() {
        for e in entries {
            let e = e.unwrap();

            // there should only be one directory in the root of a package, but this loop should work regardless
            trace!("Checking if '{}' is a directory", e.path().display());
            if e.path().is_dir() {
                trace!("It is");
                if e.path().ends_with("mods") {
                    let mut dirs = e.path().read_dir().unwrap();
                    while let Some(Ok(e)) = dirs.next() {
                        let name = e.file_name();
                        let name = name.to_str().unwrap();
                        debug!("Add submod {}", name);
                        mods.push(Path::new("mods").join(name));
                    }
                } else {
                    // sometimes people don't use the `mods` folder if they only have one mod
                    // this is technically incorrect but we should handle it anyways
                    debug!(
                        "Add one submod {}",
                        e.path().file_name().unwrap().to_string_lossy()
                    );
                    mods.push(PathBuf::new());
                }
            }
        }
    }

    if mods.is_empty() {
        return Err(ThermiteError::UnknownError(
            "Couldn't find a mod directory to copy".into(),
        ));
    }

    let manifest = temp_dir.join("manifest.json");
    let author = author.as_ref();

    let mut fin = vec![];

    // move the mod files from the temp dir to the real dir
    for submod in &mut mods {
        // the location of the mod within the temp dir
        let temp = temp_dir.join(&submod);
        // the name of the folder the mod lives in
        let p = match submod.strip_prefix("mods") {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Error striping directory prefix (this usually is caused by a misformated mod)"
                );
                debug!("{e}");
                // this behavior should maybe be configurable somehow
                // TODO: use lazystatic config value here?
                return Err(e.into());
            }
        };
        // the location of the mod within the target install dir
        let perm = mods_dir.join(p);

        let author_file = perm.join("thunderstore_author.txt");
        let manifest_file = perm.join("manifest.json");
        trace!(
            "Temp path: {} | Perm path: {}",
            temp.display(),
            perm.display()
        );

        // remove any existing files
        if perm.try_exists()? {
            fs::remove_dir_all(&perm)?;
        }
        fs::rename(&temp, &perm)?;

        // check if the manifest exists first, it may not if the mod didn't come from thunderstore
        if manifest.try_exists()? {
            fs::copy(&manifest, manifest_file)?;
        }

        // add 'thunderstore_author.txt' using the provided author name
        fs::write(author_file, author)?;

        fin.push(perm);
    }

    Ok(fin)
}

/// Install a mod to a directory
/// # Params
/// * `author` - string that identifies the package author
/// * `zip_file` - compressed mod file
/// * `target_dir` - directory to install to
///
/// `target_dir` will be treated as the root of the `mods` directory in the mod file
///
/// # Errors
/// * IO Errors
/// * Misformatted mods (typically missing the `mods` directory)
pub fn install_mod<T>(
    author: impl AsRef<str>,
    zip_file: T,
    target_dir: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, ThermiteError>
where
    T: Read + Seek,
{
    install_with_sanity(author, zip_file, target_dir, None, |_| true)
}

/// Install N* to the provided path
///
/// # Params
/// * `zip_file` - compressed mod file
/// * `game_path` - the path of the Titanfall 2 install
///
/// # Errors
/// * IO Errors
pub fn install_northstar(
    zip_file: impl Read + Seek + Copy,
    game_path: impl AsRef<Path>,
) -> Result<(), ThermiteError> {
    let target = game_path.as_ref();
    let mut archive = ZipArchive::new(zip_file)?;

    let manifest = archive
        .by_name("manifest.json")
        .ok()
        .map(|mut v| {
            let mut buf = Vec::with_capacity(usize::try_from(v.size())?);
            if let Err(e) = v.read_to_end(&mut buf) {
                Err(ThermiteError::from(e))
            } else {
                Ok(buf)
            }
        })
        .transpose()?;

    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;

        //This should work fine for N* because the dir structure *should* always be the same
        if f.enclosed_name()
            .ok_or_else(|| ThermiteError::UnknownError("File missing enclosed name".into()))?
            .starts_with("Northstar")
        {
            let out = target.join(
                f.enclosed_name()
                    .unwrap()
                    .strip_prefix("Northstar")
                    .unwrap(),
            );

            if (*f.name()).ends_with('/') {
                trace!("Create directory {}", f.name());
                fs::create_dir_all(target.join(f.name()))?;
                continue;
            } else if let Some(p) = out.parent() {
                fs::create_dir_all(p)?;
            }

            let mut outfile = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&out)?;

            trace!("Write file {}", out.display());

            io::copy(&mut f, &mut outfile)?;
        }
    }

    // add manifest and author file
    for child in game_path
        .as_ref()
        .join("R2Northstar")
        .join("mods")
        .read_dir()?
    {
        let Ok(child) = child else {
            continue;
        };
        if ![
            OsString::from("Northstar.Client"),
            OsString::from("Northstar.Custom"),
            OsString::from("Northstar.CustomServers"),
        ]
        .contains(&child.file_name())
        {
            continue;
        }

        if child.file_type()?.is_dir() {
            let dir = child.path();
            let manifest_file = dir.join("manifest.json");
            let author_file = dir.join("thunderstore_author.txt");

            // write the manifest to the mod's directory
            {
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(manifest_file)?;
                if let Some(manifest) = &manifest {
                    file.write_all(manifest)?;
                }
            }

            // write the author file to the mod's directory
            {
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(author_file)?;
                file.write_all(b"northstar")?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {

    use mockall::mock;
    use std::io::Cursor;

    use super::{install_mod, *};

    mock! {
        Writer {}
        impl Write for Writer {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
            fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
            fn flush(&mut self) -> io::Result<()>;
        }

    }

    mock! {
        Archive {}
        impl Read for Archive {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
        }

        impl Seek for Archive {
            fn seek(&mut self, pos: std::io::SeekFrom) -> io::Result<u64>;
        }
    }

    const TEST_URL: &str =
        "https://freetestdata.com/wp-content/uploads/2023/04/2.4KB_JSON-File_FreeTestData.json";
    const TEST_SIZE_BYTES: u64 = 2455;

    const TEST_ARCHIVE: &[u8] = include_bytes!("test_archive.zip");

    #[test]
    fn download_file() {
        let mut mock_writer = MockWriter::new();
        mock_writer
            .expect_write_all()
            .returning(|_| Ok(()))
            .times((TEST_SIZE_BYTES as usize / super::CHUNK_SIZE)..);

        let res = download(mock_writer, TEST_URL);
        assert!(res.is_ok());
        res.map(|size| {
            assert_eq!(size, TEST_SIZE_BYTES);
            size
        })
        .unwrap();
    }

    #[test]
    fn fail_insanity() {
        let archive = MockArchive::new();
        let res = install_with_sanity("test", archive, ".", None, |_| return false);

        assert!(res.is_err());
        match res {
            Err(ThermiteError::SanityError) => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn install() {
        let mut cursor = Cursor::new(TEST_ARCHIVE);
        let target_dir = TempDir::create("./test_dir").expect("Unabel to create temp dir");
        let res = install_mod("test", &mut cursor, &target_dir);

        assert!(res.is_ok());
        let res = res.unwrap();
        assert!(!res.is_empty());
        assert_eq!(target_dir.join("Smart CAR").canonicalize().unwrap(), res[0]);

        let path = &res[0];

        assert!(path.try_exists().unwrap());
        assert!(
            path.join("mod.json").try_exists().unwrap(),
            "mod.json should exist"
        );
        assert!(
            path.join("manifest.json").try_exists().unwrap(),
            "manifest.json should exist"
        );
        assert!(
            path.join("thunderstore_author.txt").try_exists().unwrap(),
            "thunderstore_author.txt should exist"
        );
        let author = fs::read_to_string(path.join("thunderstore_author.txt")).unwrap();

        assert_eq!("test", author);
    }
}
