# thermite
Rust crate for managing Northstar and interacting with Thunderstore

### v0.6.1
- Update proton feature to use the new download functions

### v0.6.0
- rename `download_file` and `download_file_with_progress` to `download` and `download_with_progress`
- make download and install functions more generic
  - download functions now accept an output `Write`r to write to 
  - install functions now accept types implementing `Read + Seek`

### v0.5.2
- add `proton` feature with `latest_release`, `download_ns_proton`, and `install_ns_proton` functions
- change dependency filter to make sure Northstar itself doesn't slip through
- properly add `manifest.json` and `thunderstore_author.txt` to the Northstar base mods

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
