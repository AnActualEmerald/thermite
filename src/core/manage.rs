use std::{
    error::Error,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use crate::error::{Result, ThermiteError};

use zip::ZipArchive;

use tracing::{debug, trace, warn};

use super::utils::validate_modstring;

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
pub fn download_with_progress<F>(mut output: impl Write, url: impl AsRef<str>, cb: F) -> Result<u64>
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
pub fn download(output: impl Write, url: impl AsRef<str>) -> Result<u64> {
    download_with_progress(output, url, |_, _, _| {})
}

#[deprecated(since = "0.7.1", note = "just use std::fs directly")]
pub fn uninstall(mods: &[impl AsRef<Path>]) -> Result<()> {
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
    mod_string: impl AsRef<str>,
    zip_file: T,
    target_dir: impl AsRef<Path>,
    sanity_check: F,
) -> Result<PathBuf>
where
    T: Read + Seek,
    F: FnOnce(&T) -> Result<(), Box<dyn Error + Send + Sync + 'static>>,
{
    if let Err(e) = sanity_check(&zip_file) {
        return Err(ThermiteError::SanityError(e));
    }

    if !validate_modstring(mod_string.as_ref()) {
        return Err(ThermiteError::NameError(mod_string.as_ref().into()));
    }

    let path = target_dir.as_ref().join(mod_string.as_ref());
    ZipArchive::new(zip_file)?.extract(&path)?;

    Ok(path)
}

pub fn install_mod<T>(
    mod_string: impl AsRef<str>,
    zip_file: T,
    target_dir: impl AsRef<Path>,
) -> Result<PathBuf>
where
    T: Read + Seek,
{
    install_with_sanity(mod_string, zip_file, target_dir, |_| Ok(()))
}

/// Install N* to the provided path
///
/// # Params
/// * `zip_file` - compressed mod file
/// * `game_path` - the path of the Titanfall 2 install
///
/// # Errors
/// * IO Errors
pub fn install_northstar(zip_file: impl Read + Seek, game_path: impl AsRef<Path>) -> Result<()> {
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
                    .expect("enclosed name")
                    .strip_prefix("Northstar")
                    .expect("Nortstar prefix"),
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

    use crate::core::utils::TempDir;
    use mockall::mock;
    use std::io::Cursor;
    use tracing::info;

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

    const TEST_ARCHIVE: &[u8] = include_bytes!("test_media/test_archive.zip");
    const TEST_NS_ARCHIVE: &[u8] = include_bytes!("test_media/northstar.zip");

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
        let res = install_with_sanity("foo-bar-0.1.0", archive, ".", |_| {
            Err(Box::new(ThermiteError::UnknownError("uh oh".into())))
        });

        assert!(res.is_err());
        match res {
            Err(ThermiteError::SanityError(_)) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn fail_invalid_name() {
        let archive = MockArchive::new();
        let res = install_mod("invalid", archive, ".");

        if let Err(ThermiteError::NameError(name)) = res {
            assert_eq!(name, "invalid");
        }
    }

    #[test]
    fn install() {
        let mut cursor = Cursor::new(TEST_ARCHIVE);
        let path = TempDir::create("./test_dir").expect("Unable to create temp dir");
        let res = install_mod("foo-bar-0.1.0", &mut cursor, &path);

        if let Ok(path) = res {
            assert!(
                path.join("mods")
                    .join("Smart CAR")
                    .join("mod.json")
                    .try_exists()
                    .unwrap(),
                "mod.json should exist"
            );
            assert!(
                path.join("manifest.json").try_exists().unwrap(),
                "manifest.json should exist"
            );
        } else {
            panic!("Install failed with {:?}", res);
        }
    }

    #[test]
    fn northstar() {
        let mut cursor = Cursor::new(TEST_NS_ARCHIVE);
        let path = TempDir::create("./northstar_test").expect("Create temp dir");
        std::fs::create_dir_all(&path).expect("create dir");
        let res = install_northstar(&mut cursor, &path);

        info!("{:?}: {}", path, path.exists());
        info!("{res:?}");

        if res.is_ok() {
            assert!(
                path.join("NorthstarLauncher.exe").try_exists().unwrap(),
                "NorthstarLauncher should exist"
            );

            assert!(
                path.join("R2Northstar")
                    .join("mods")
                    .join("Northstar.Client")
                    .try_exists()
                    .unwrap(),
                "Northstar client mod should exist"
            );
        } else {
            panic!("Install failed with {:?}", res);
        }
    }
}
