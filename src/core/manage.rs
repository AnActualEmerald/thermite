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
    url: impl AsRef<str>,
    file_path: impl AsRef<Path>,
    pb: impl Into<Option<ProgressBar>>,
) -> Result<File, ThermiteError> {
    let client = Client::new();
    let pb = pb.into();
    let file_path = file_path.as_ref();

    //send the request
    let res = client.get(url.as_ref()).send().await?;

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
    debug!("Downloading file of size: {}", file_size);

    //start download in chunks
    let mut file = File::create(file_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();
    debug!("Starting download from {}", url.as_ref());
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
pub async fn download_file(
    url: impl AsRef<str>,
    file_path: impl AsRef<Path>,
) -> Result<File, ThermiteError> {
    download_file_with_progress(url, file_path.as_ref(), None).await
}

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
/// * zip_file - compressed mod file
/// * target_dir - directory to install to
/// * extract_dir - directory to extract to before installing. Defaults to a temp directory in `target_dir`
/// * sanity_check - function that will be called before performing the installation. The operation will fail with `ThermiteError::SanityError` if this returns `false`
///     - takes `File` of the zip file
///     - returns `bool`
///
/// `target_dir` will be treated as the root of the `mods` directory in the mod file
pub fn install_with_sanity<F>(
    author: impl AsRef<str>,
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
        return Err(ThermiteError::MiscError(
            "Couldn't find a mod directory to copy".into(),
        ));
    }

    let manifest = temp_dir.join("manifest.json");
    let author = author.as_ref();

    // move the mod files from the temp dir to the real dir
    for submod in mods.iter_mut() {
        // the location of the mod within the temp dir
        let temp = temp_dir.join(&submod);
        // the name of the folder the mod lives in
        let p = submod.strip_prefix("mods")?;
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
    author: impl AsRef<str>,
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
