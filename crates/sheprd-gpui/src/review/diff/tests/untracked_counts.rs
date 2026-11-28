use super::*;

#[test]
fn untracked_text_files_are_counted_and_binaries_and_budgets_are_respected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.path().join("no_eol.txt"), "alpha\nbeta").unwrap();
    std::fs::write(dir.path().join("crlf.txt"), "a\r\nb\r\n").unwrap();
    std::fs::write(dir.path().join("empty.txt"), "").unwrap();
    std::fs::write(dir.path().join("image.bin"), b"\x89PNG\0binary").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/hosts", dir.path().join("link.txt")).unwrap();

    let mut files = vec![
        FileDiff::new("notes.txt".into(), Status::Untracked),
        FileDiff::new("no_eol.txt".into(), Status::Untracked),
        FileDiff::new("crlf.txt".into(), Status::Untracked),
        FileDiff::new("empty.txt".into(), Status::Untracked),
        FileDiff::new("image.bin".into(), Status::Untracked),
        #[cfg(unix)]
        FileDiff::new("link.txt".into(), Status::Untracked),
    ];
    let mut loaded = Loaded {
        source: load::Source::local(&dir.path().to_string_lossy()),
        scope: Scope::Uncommitted,
        base: None,
        diff: Diff {
            files: std::mem::take(&mut files),
            before: "HEAD".into(),
        },
    };
    count_untracked(&mut loaded);
    let counts: Vec<_> = loaded
        .diff
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.added, file.removed))
        .collect();
    #[cfg(unix)]
    assert_eq!(
        counts,
        vec![
            ("notes.txt", Some(3), Some(0)),
            ("no_eol.txt", Some(2), Some(0)),
            ("crlf.txt", Some(2), Some(0)),
            ("empty.txt", Some(0), Some(0)),
            ("image.bin", None, None),
            ("link.txt", None, None),
        ]
    );
    #[cfg(not(unix))]
    assert_eq!(
        counts,
        vec![
            ("notes.txt", Some(3), Some(0)),
            ("no_eol.txt", Some(2), Some(0)),
            ("crlf.txt", Some(2), Some(0)),
            ("empty.txt", Some(0), Some(0)),
            ("image.bin", None, None),
        ]
    );
}

#[test]
fn untracked_counting_stops_at_file_and_byte_budgets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "1234\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "5678\n").unwrap();
    std::fs::write(dir.path().join("c.txt"), "9012\n").unwrap();

    // File budget caps at 1 file.
    let mut by_files = vec![
        FileDiff::new("a.txt".into(), Status::Untracked),
        FileDiff::new("b.txt".into(), Status::Untracked),
    ];
    load::count_untracked_in(dir.path(), &mut by_files, 1, 1024);
    assert_eq!((by_files[0].added, by_files[0].removed), (Some(1), Some(0)));
    assert_eq!((by_files[1].added, by_files[1].removed), (None, None));

    // Byte budget of 6 bytes fits a.txt (5 bytes) and leaves 1 byte, skipping b.txt and c.txt.
    let mut by_bytes = vec![
        FileDiff::new("a.txt".into(), Status::Untracked),
        FileDiff::new("b.txt".into(), Status::Untracked),
        FileDiff::new("c.txt".into(), Status::Untracked),
    ];
    load::count_untracked_in(dir.path(), &mut by_bytes, 10, 6);
    assert_eq!((by_bytes[0].added, by_bytes[0].removed), (Some(1), Some(0)));
    assert_eq!((by_bytes[1].added, by_bytes[1].removed), (None, None));
    assert_eq!((by_bytes[2].added, by_bytes[2].removed), (None, None));
}
