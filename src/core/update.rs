use std::{fs, path::Path};

use log::{debug, trace};

use crate::{
    error::ThermiteError,
    prelude::{LocalIndex, Mod},
};

use super::{actions, Ctx};

/// Download and install updated versions of provided mods. Updates the `LocalIndex` and clears old versions from the cache as well.
/// # Params
/// * ctx - the current context
/// * outdated - the mods to update
/// * target - the index file to target
pub async fn update(
    ctx: &mut Ctx,
    outdated: &[Mod],
    target: &mut LocalIndex,
) -> Result<(), ThermiteError> {
    let mut downloaded = vec![];
    for base in outdated {
        let name = &base.name;
        let url = &base.url;
        let path = ctx
            .dirs
            .cache_dir()
            .join(format!("{}_{}.zip", name, base.version));
        match actions::download_file(url, path).await {
            Ok(f) => downloaded.push(f),
            Err(e) => eprintln!("{}", e),
        }
    }

    for f in downloaded.into_iter() {
        let mut pkg = actions::install_mod(&f, target.path().as_ref()).unwrap();
        ctx.cache.clean(&pkg.package_name, &pkg.version)?;
        let dir = target.parent_dir();
        target.mods.entry(pkg.package_name).and_modify(|inst| {
            inst.version = pkg.version;
            //Don't know if sorting is needed here but seems like a good assumption
            inst.mods
                .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            pkg.mods
                .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            for (curr, new) in inst.mods.iter().zip(pkg.mods.iter()) {
                trace!("current mod: {:#?} | new mod: {:#?}", curr, new);
                if curr.disabled() {
                    fs::remove_dir_all(dir.join(&curr.path)).unwrap();
                    debug!(
                        "Moving mod from {} to {}",
                        new.path.display(),
                        curr.path.display()
                    );
                    fs::rename(dir.join(&new.path), dir.join(&curr.path)).unwrap_or_else(|e| {
                        debug!("Unable to move sub-mod to old path");
                        debug!("{}", e);
                    });
                }
            }

            debug!("Updated {}", inst.package_name);
        });
    }

    Ok(())
}

/// Finds mods in the `LocalIndex` whose version doesn't match the provided remote index
/// # Params
/// * index - a list of `Mod`s. Should be retreived from thermite::update_index.
/// * target - the `LocalIndex` to check against
pub async fn get_outdated(index: &[Mod], target: &LocalIndex) -> Vec<Mod> {
    index
        .iter()
        .filter(|e| {
            target
                .mods
                .iter()
                .any(|(n, i)| n.trim() == e.name.trim() && i.version.trim() != e.version.trim())
        })
        .cloned()
        .collect()
}
