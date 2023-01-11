use std::{
    cmp::min,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{core::utils::TempDir, error::ThermiteError};

use futures_util::StreamExt;
use indicatif::ProgressBar;

use reqwest::Client;
use zip::ZipArchive;

use log::{debug, error, trace};

/// Download a file and update a progress bar
/// # Params
/// * url - URL to download from
/// * file_path - Full path to save file to
/// * pb - `ProgressBar` to update
pub async fn download_file_with_progress(
    url: &str,
    file_path: impl AsRef<Path>,
    pb: impl Into<Option<ProgressBar>>,
) -> Result<File, ThermiteError> {
    let client = Client::new();
    let pb = pb.into();
    let file_path = file_path.as_ref();

    //send the request
    let res = client.get(url).send().await?;

    if !res.status().is_success() {
        error!("Got bad response from thunderstore");
        error!("{:?}", res);
        return Err(ThermiteError::MiscError(format!(
            "Thunderstore returned error: {:#?}",
            res
        )));
    }

    let file_size = res
        .content_length()
        .ok_or_else(|| ThermiteError::MiscError("Missing content length header".into()))?;
    debug!("Downloading file size: {}", file_size);

    //start download in chunks
    let mut file = File::create(file_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();
    debug!("Starting download from {}", url);
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        if let Some(pb) = &pb {
            let new = min(downloaded + (chunk.len() as u64), file_size);
            downloaded = new;
            pb.set_position(new);
        }
    }
    let finished = File::open(file_path)?;
    debug!("Finished download to {}", file_path.display());

    if let Some(pb) = &pb {
        pb.finish_with_message(format!(
            "Downloaded {}!",
            file_path.file_name().unwrap().to_string_lossy()
        ));
    }
    Ok(finished)
}

/// Wrapper for calling `download_file_with_progress` without a progress bar
/// # Params
/// * url - Url to download from
/// * file_path - Full path to save file to
pub async fn download_file(url: &str, file_path: impl AsRef<Path>) -> Result<File, ThermiteError> {
    download_file_with_progress(url, file_path.as_ref(), None).await
}

pub fn uninstall(mods: Vec<&PathBuf>) -> Result<(), ThermiteError> {
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
/// * zip_file - compressed mod file
/// * target_dir - directory to install to
/// * extract_dir - directory to extract to before installing. Defaults to a temp directory in `target_dir`
/// * sanity_check - function that will be called before performing the installation. The operation will fail with `ThermiteError::SanityError` if this returns `false`
///     - takes `File` of the zip file
///     - returns `bool`
///
/// `target_dir` will be treated as the root of the `mods` directory in the mod file
pub fn install_with_sanity<'a, F>(
    author: impl Into<&'a str>,
    zip_file: &File,
    target_dir: impl AsRef<Path>,
    extract_dir: Option<&Path>,
    sanity_check: F,
) -> Result<(), ThermiteError>
where
    F: FnOnce(&File) -> bool,
{
    let target_dir = target_dir.as_ref();
    if !sanity_check(zip_file) {
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
                .unwrap()
                .as_secs()
                .to_string(),
        )
    };

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
    if let Ok(entries) = temp_dir.path.read_dir() {
        for e in entries {
            let e = e.unwrap();

            if e.path().is_dir() {
                //Get the path relative to the .papa.ron file
                // let m_path = e.path();
                // let m_path = m_path.strip_prefix(&temp_dir)?;
                if e.path().ends_with("mods") {
                    let mut dirs = e.path().read_dir().unwrap();
                    while let Some(Ok(e)) = dirs.next() {
                        let name = e.file_name();
                        let name = name.to_str().unwrap();
                        debug!("Add submod {}", name);
                        mods.push(Path::new("mods").join(name));
                    }
                } else {
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
        return Err(ThermiteError::MiscError(
            "Couldn't find a directory to copy".into(),
        ));
    }

    let author = author.into();
    // move the mod files from the temp dir to the real dir
    for p in mods.iter_mut() {
        let temp = temp_dir.path.join(&p);
        let p = p.strip_prefix("mods")?;
        let perm = mods_dir.join(p);
        let author_file = perm.join("thunderstore_author.txt");
        trace!(
            "Temp path: {} | Perm path: {}",
            temp.display(),
            perm.display()
        );

        if perm.exists() {
            fs::remove_dir_all(&perm)?;
        }
        fs::rename(&temp, &perm)?;
        fs::write(author_file, &author)?;
    }

    Ok(())
}

/// Install a mod to a directory
/// # Params
/// * author - string that identifies the package author
/// * zip_file - compressed mod file
/// * target_dir - directory to install to
///
/// `target_dir` will be treated as the root of the `mods` directory in the mod file
pub fn install_mod<'a>(
    author: impl Into<&'a str>,
    zip_file: &File,
    target_dir: impl AsRef<Path>,
) -> Result<(), ThermiteError> {
    install_with_sanity(author, zip_file, target_dir, None, |_| true)
}

/// Install N* to the provided path
///
/// # Params
/// * zip_file - compressed mod file
/// * game_path - the path of the Titanfall 2 install
pub async fn install_northstar(
    zip_file: &File,
    game_path: impl AsRef<Path>,
) -> Result<(), ThermiteError> {
    let target = game_path.as_ref();
    let mut archive = ZipArchive::new(zip_file)?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).unwrap();

        //This should work fine for N* because the dir structure *should* always be the same
        if f.enclosed_name().unwrap().starts_with("Northstar") {
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

    Ok(())
}
