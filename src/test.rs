use crate::api;
use crate::resolve_deps;

#[test]
fn get_packages_from_tstore() {
    let index = tokio_test::block_on(api::get_package_index());
    assert!(index.is_ok());
    let index = index.unwrap();
    assert!(!index.is_empty());
    let mut deps = 0;
    for f in index {
        for d in f.versions.get(&f.latest).unwrap().deps.iter() {
            assert_ne!(d, "northstar-Northstar");
            deps += 1;
        }
    }

    assert_ne!(0, deps);
}

#[test]
fn resolve_dependencies() {
    let index = tokio_test::block_on(api::get_package_index()).unwrap();
    if let Some(md) = index.iter().find(|e| e.name == "mp_mirror_city") {
        let deps = resolve_deps(&md.get_latest().unwrap().deps, &index);
        assert!(deps.is_ok());
        assert_ne!(deps.unwrap().len(), 0);
    }
}
