# thermite
Rust crate for managing Northstar and interacting with Thunderstore

### v0.4.0
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
