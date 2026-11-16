//! Ren'Py 7 game-pack regression: Python 2.7 bytecode is game data and must be
//! preserved, while desktop platform binaries remain excluded.

use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_pack::{GpackPacker, GpackReader};
use std::fs;
use std::path::{Path, PathBuf};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nonce))
}

#[test]
fn renpy7_stdlib_pyo_is_packed_and_desktop_binaries_are_excluded() {
    let root = unique_temp_dir("rtp_renpy7_pack_src");
    for rel in [
        "main.py",
        "renpy/__init__.py",
        "lib/python2.7/os.pyo",
        "lib/python2.7/lib-dynload/empty.txt",
        "lib/py2-windows-x86_64/python27.dll",
        "lib/py2-windows-x86_64/y.dll",
        "game/script.rpyc",
    ] {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("fixture:{rel}")).unwrap();
    }
    let pyo_bytes = b"\x03\xf3\x0d\x0a-fixture-pyo-bytes";
    fs::write(root.join("lib/python2.7/os.pyo"), pyo_bytes).unwrap();

    let index = GameSourceIndex::build(&root).expect("fixture source index");
    let exclusions = GpackPacker::compute_packaging_exclusions(&index);
    let excluded: Vec<&str> = exclusions
        .entries
        .iter()
        .map(|entry| entry.logical_path.as_str())
        .collect();
    assert!(
        excluded.contains(&"lib/py2-windows-x86_64/y.dll"),
        "desktop Windows payload must be excluded, got {excluded:?}"
    );
    assert!(
        excluded.contains(&"lib/py2-windows-x86_64/python27.dll"),
        "desktop Python DLL must be excluded, got {excluded:?}"
    );
    assert!(
        !excluded.iter().any(|path| path.starts_with("lib/python2.7/")),
        "Python 2.7 stdlib must not be swept into desktop exclusions, got {excluded:?}"
    );

    let pack = unique_temp_dir("rtp_renpy7_pack_out").join("game.pack");
    fs::create_dir_all(pack.parent().unwrap()).unwrap();
    GpackPacker::pack_from_index(&index, &pack).expect("pack fixture");

    let mut reader = GpackReader::open(&pack).expect("open fixture pack");
    assert!(
        reader.has_file("lib/python2.7/os.pyo"),
        "packed game must preserve lib/python2.7/*.pyo"
    );
    assert_eq!(
        reader.read_file("lib/python2.7/os.pyo").unwrap(),
        pyo_bytes,
        ".pyo bytes must be preserved verbatim"
    );
    assert!(
        !reader.has_file("lib/py2-windows-x86_64/y.dll"),
        "packed game must not contain desktop Windows payload"
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(Path::new(&pack).parent().unwrap());
}
