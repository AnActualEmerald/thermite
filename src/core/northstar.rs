use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

use log::{debug, trace};
use zip::ZipArchive;

use crate::{core::actions, error::ThermiteError, model::Cache, ModVersion};

use super::{utils, Ctx};

///Install N* to the provided path
///
///Returns the version that was installed
pub async fn install_northstar(ctx: Ctx, game_path: &Path) -> Result<String, ThermiteError> {
    let index = utils::update_index::<&Path>(None, None).await;
    let nmod = index
        .iter()
        .find(|f| f.name.to_lowercase() == "northstar")
        .ok_or_else(|| {
            ThermiteError::MiscError("Unable to find Northstar in Thunderstore index".to_string())
        })?;

    do_install(
        &ctx.cache,
        nmod.versions.get(&nmod.latest).unwrap(),
        game_path,
    )
    .await?;

    Ok(nmod.latest.clone())
}

///Install N* from the provided mod
///
///Checks cache, else downloads the latest version
async fn do_install(
    cache: &Cache,
    nmod: &ModVersion,
    game_path: &Path,
) -> Result<(), ThermiteError> {
    let filename = format!("northstar-{}.zip", nmod.version);
    let nfile = if let Some(f) = cache.check(Path::new(&filename)) {
        debug!("Using cached version of Northstar");
        f
    } else {
        actions::download_file(&nmod.url, cache.path().join(&filename)).await?
    };
    debug!("Extracting Northstar...");
    extract(&nfile, game_path)?;

    Ok(())
}

///Extract N* zip file to target game path
fn extract(zip_file: &File, target: &Path) -> Result<(), ThermiteError> {
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
                fs::create_dir_all(&p)?;
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
