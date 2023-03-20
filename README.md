# thermite
Rust crate for managing Northstar and interacting with Thunderstore

### v0.6.0
- rename `download_file` and `download_file_with_progress` to `download` and `download_with_progress`
- make download and install functions more generic
  - download functions now return `Vec<u8>` rather than `File`
  - install functions now accept types implementing `Read + Seek`

### v0.5.0
- change `install_with_progress` to take a function rather than an indicatif bar
- change to `ureq` for http requests, making all functions synchronus
- add `InstalledMod` struct which contains a mod's `mod.json`, `manifest.json`, `thunderstore_author.txt`, and the path of the root directory
- change `find_mods` to return a vec of `InstalledMod`


### v0.4.0
- move exported functions from crate root to `prelude` module
- add `manifest.json` and `thunderstore_author.txt` to installed packages
- add optional sanity check for install function
- add `find_mods` function
- add `enabledmods.json` support
- remove `Ctx`, `LocalIndex`, `LocalMod` `Cache`, `CachedMod`

### v0.3.4
- fix panic for missing `enclosed_name` when installing mods
- fix mod dependencies missing

### v0.3.3
- fix StripPrefixError panic
- ensure temp directories are cleaned up
