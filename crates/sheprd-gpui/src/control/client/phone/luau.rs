//! `phone luau` and `phone snapshot`: an agent changing a Luau screen of
//! the phone's app while it runs, and seeing what the phone shows.
use super::{Asked, NO_PHONE, drive, output};
use crate::control::{
    ErrorCode,
    client::{Context, Output, unexpected},
    protocol::{Request, Response, phone::Answer},
};
use sheprd_remote_control::control::phone::{self, Command, LuauReport, Outcome, Phone};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// How often `--watch` looks at the folder again.
const POLL: Duration = Duration::from_millis(300);

pub(super) fn luau(
    device: Option<String>,
    dir: &Path,
    screen: Option<String>,
    watch: bool,
    reset: bool,
    context: &Context,
) -> crate::Result<Output> {
    let screen = match screen {
        Some(screen) => screen,
        None => screen_of(dir)?,
    };
    if reset {
        return push(device, &screen, BTreeMap::new(), context);
    }
    if !watch {
        return push(device, &screen, read(dir)?, context);
    }
    // Until stopped: push each time a file changes, and say what the phone
    // found, so an agent watching sees each save land.
    let mut last = None;
    loop {
        let files = read(dir)?;
        let stamp = stamp(dir, &files);
        if last.as_ref() != Some(&stamp) {
            last = Some(stamp);
            match push(device.clone(), &screen, files, context) {
                Ok(output) => println!("{}", output.text()),
                Err(error) => eprintln!("{error}"),
            }
        }
        std::thread::sleep(POLL);
    }
}

fn push(
    device: Option<String>,
    screen: &str,
    files: BTreeMap<String, String>,
    context: &Context,
) -> crate::Result<Output> {
    if !phone::valid_files(&files) {
        return Err(rejected(format!(
            "At most {} .luau files and {} KiB, named as a screen's folder names them",
            phone::MAX_LUAU_FILES,
            phone::MAX_LUAU_BYTES / 1024
        )));
    }
    let ask = drive(
        device,
        Command::Luau {
            screen: screen.to_owned(),
            files,
        },
    );
    match context.call(Request::Phone, ask)? {
        Response::Phone { answer } => match &answer {
            Answer::Driven {
                phone,
                outcome: Outcome::Luau { report },
            } => {
                let text = reported(phone, screen, report);
                let output = Output::data(text, &answer)?;
                Ok(if report.problems.is_empty() {
                    output
                } else {
                    output.failed()
                })
            }
            _ => output(&answer, Asked::Show),
        },
        response => Err(unexpected(response)),
    }
}

/// What the phone found, as a person reads it.
fn reported(phone: &Phone, screen: &str, report: &LuauReport) -> String {
    let name = &phone.name;
    let mut lines = vec![if report.problems.is_empty() && report.shown {
        format!("{screen} reloaded on {name}")
    } else if report.problems.is_empty() {
        format!("{screen} loads on {name}; it shows when the screen opens")
    } else {
        format!("{screen} did not load on {name}; it keeps the last version that did")
    }];
    lines.extend(
        report
            .problems
            .iter()
            .map(|line| format!("problem: {line}")),
    );
    lines.extend(
        report
            .warnings
            .iter()
            .map(|line| format!("warning: {line}")),
    );
    lines.extend(report.logs.iter().map(|line| format!("log: {line}")));
    lines.join("\n")
}

pub(super) fn snapshot(
    device: Option<String>,
    out: Option<PathBuf>,
    context: &Context,
) -> crate::Result<Output> {
    match context.call(Request::Phone, drive(device, Command::Snapshot))? {
        Response::Phone { answer } => match &answer {
            Answer::Snapshot { phone, path } => {
                let Some(out) = out else {
                    return Output::data(path.clone(), &answer);
                };
                std::fs::copy(path, &out).map_err(|error| {
                    rejected(format!("Could not write {}: {error}", out.display()))
                })?;
                Output::data(format!("{} from {}", out.display(), phone.name), &answer)
            }
            Answer::NoPhone => Ok(Output::data(NO_PHONE, &answer)?.nothing()),
            _ => output(&answer, Asked::Show),
        },
        response => Err(unexpected(response)),
    }
}

/// The screen a folder is: its name, as the app's bundle names it.
fn screen_of(dir: &Path) -> crate::Result<String> {
    let name = std::fs::canonicalize(dir)
        .ok()
        .and_then(|dir| {
            dir.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    if phone::valid_screen(&name) {
        Ok(name)
    } else {
        Err(rejected(format!(
            "Name the screen with --screen: the folder's name, {name:?}, is not one"
        )))
    }
}

/// Every `.luau` file under `dir`, at most two folders down, by its path
/// from `dir`.
fn read(dir: &Path) -> crate::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    walk(dir, dir, 0, &mut files)?;
    if files.is_empty() {
        return Err(rejected(format!("No .luau files in {}", dir.display())));
    }
    Ok(files)
}

fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    files: &mut BTreeMap<String, String>,
) -> crate::Result<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| rejected(format!("Could not read {}: {error}", dir.display())))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        if hidden {
            continue;
        }
        if path.is_dir() && depth < 2 {
            walk(root, &path, depth + 1, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "luau")
        {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let name = relative.to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&path)
                .map_err(|error| rejected(format!("Could not read {}: {error}", path.display())))?;
            files.insert(name, text);
        }
    }
    Ok(())
}

/// What changes when a file is saved, added or removed.
fn stamp(dir: &Path, files: &BTreeMap<String, String>) -> Vec<(String, Option<SystemTime>, usize)> {
    files
        .iter()
        .map(|(name, text)| {
            let modified = std::fs::metadata(dir.join(name))
                .and_then(|meta| meta.modified())
                .ok();
            (name.clone(), modified, text.len())
        })
        .collect()
}

fn rejected(message: String) -> crate::Error {
    crate::control::Error::Rejected {
        code: ErrorCode::InvalidRequest,
        message,
    }
    .into()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_read_as_a_screen_names_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pull-requests");
        std::fs::create_dir_all(root.join("components")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("main.luau"), "return 1").unwrap();
        std::fs::write(root.join("components/row.luau"), "return 2").unwrap();
        std::fs::write(root.join("notes.md"), "not Luau").unwrap();
        std::fs::write(root.join(".git/x.luau"), "hidden").unwrap();

        let files = read(&root).unwrap();
        assert_eq!(
            files.keys().collect::<Vec<_>>(),
            ["components/row.luau", "main.luau"]
        );
        assert!(phone::valid_files(&files));
        assert_eq!(screen_of(&root).unwrap(), "pull-requests");
    }

    #[test]
    fn a_report_says_whether_it_loaded() {
        let phone = Phone {
            device: sheprd_remote_control::control::DeviceId("d".into()),
            name: "iPhone".into(),
            active_ms: 0,
        };
        let report = LuauReport {
            shown: true,
            problems: vec!["main.luau:3: oops".into()],
            ..LuauReport::default()
        };
        let text = reported(&phone, "pull-requests", &report);
        assert!(text.starts_with("pull-requests did not load on iPhone"));
        assert!(text.contains("problem: main.luau:3: oops"));
    }
}
