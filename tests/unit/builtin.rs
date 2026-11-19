use super::{normalize, resolve_cd};
use std::path::Path;

#[test]
fn normalize_preserves_relative_parent_components() {
    assert_eq!(normalize(Path::new("../../a/../b")), Path::new("../../b"));
    assert_eq!(normalize(Path::new("a/../../b")), Path::new("../b"));
}

#[cfg(windows)]
#[test]
fn normalize_keeps_windows_roots() {
    assert_eq!(normalize(Path::new(r"C:\..\..")), Path::new("C:\\"));
    assert_eq!(normalize(Path::new(r"D:\foo\..\..")), Path::new("D:\\"));
    assert_eq!(
        normalize(Path::new(r"\\server\share\..\dir")),
        Path::new(r"\\server\share\dir")
    );
}

#[cfg(windows)]
#[test]
fn resolve_cd_switches_drives_and_keeps_paths() {
    let cwd = Path::new(r"D:\Work\WorkPlan\C\kria\Terminal");
    assert_eq!(resolve_cd(cwd, Path::new("C:\\")), Path::new("C:\\"));
    assert_eq!(
        resolve_cd(Path::new("C:\\"), Path::new(r"D:\Work\WorkPlan")),
        Path::new(r"D:\Work\WorkPlan")
    );
}
