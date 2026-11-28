//! Which repository a Herdr 1 workspace's worktree requests and lookups name.

// i18n: migrated
use sheprd_client::protocol::{ClientShellSnapshot, ClientShellWorkspace, ClientShellWorktree};

/// How a worktree request tells the daemon which repository it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::menu) enum Source {
    /// By workspace: the daemon branches the repository it keeps for it.
    Workspace,
    /// By directory, which the daemon resolves to its repository. Herdr keeps
    /// the repository a workspace was first given while its directory, and the
    /// label drawn from it, follow the shell: a workspace Teleport opened on
    /// one repository whose shell then moved into another would otherwise
    /// branch the first one under the second one's name.
    Directory(String),
    /// A repository's main checkout directory, for a linked checkout whose
    /// only open main checkout's shell has wandered off: the daemon refuses
    /// a linked checkout as a source. Its Git directory, with the linked
    /// checkout's branch, is what lookups read, since the main checkout's
    /// reported branch is of the repository its shell moved to. Also for a
    /// main checkout whose focused pane wandered off while its first tab's
    /// root pane still stands in the kept repository.
    Repository { root: String, key: String },
}

impl Source {
    pub(in crate::menu) fn of(
        snapshot: &ClientShellSnapshot,
        workspace: &ClientShellWorkspace,
    ) -> Self {
        if !left_checkout(workspace) {
            return Self::Workspace;
        }
        match stays_in_repository(snapshot, workspace) {
            Some((root, key)) => Self::Repository {
                root: root.to_owned(),
                key: key.to_owned(),
            },
            None => Self::Directory(workspace.new_workspace_cwd.clone()),
        }
    }
}

/// Whether `workspace` is a main checkout whose directory is no longer inside
/// the repository Herdr keeps for it. A linked checkout's own path is not in
/// the snapshot, so it is never judged.
pub(super) fn left_checkout(workspace: &ClientShellWorkspace) -> bool {
    workspace.worktree.as_ref().is_some_and(|tree| {
        !tree.is_linked_worktree
            && crate::worktree::left_checkout(&tree.key, &workspace.new_workspace_cwd)
    })
}

/// The repository `workspace` is in, as far as it can be trusted: none for a
/// main checkout whose shell has left the repository Herdr keeps for it.
pub(in crate::menu) fn known_worktree(
    workspace: &ClientShellWorkspace,
) -> Option<ClientShellWorktree> {
    workspace
        .worktree
        .clone()
        .filter(|_| !left_checkout(workspace))
}

/// The root and Git directory of the repository a main checkout whose
/// directory left it still stands in. The workspace's directory follows the
/// active tab's focused pane, while its identity follows the first tab's root
/// pane: when that first surviving pane still sits inside the kept repository
/// (and any automatic label still names it), only another pane wandered off (a
/// chat started in the home folder, say), and the repository is the one to
/// branch. When the root shell itself moved into another repository — even one
/// sharing the same folder name — its pane directory lies outside the kept
/// checkout, so the current directory is named instead.
fn stays_in_repository<'a>(
    snapshot: &'a ClientShellSnapshot,
    workspace: &'a ClientShellWorkspace,
) -> Option<(&'a str, &'a str)> {
    let tree = workspace
        .worktree
        .as_ref()
        .filter(|tree| !tree.is_linked_worktree)?;
    let root = crate::worktree::checkout_root(&tree.key)?;
    let root_cwd = crate::palette::launch_root(snapshot, &workspace.workspace_id)
        .filter(|cwd| !cwd.is_empty())?;
    if crate::worktree::left_checkout(&tree.key, root_cwd) {
        return None;
    }
    if !workspace.custom_label {
        let name = path_basename(root);
        let cwd_name = path_basename(root_cwd);
        if name.is_empty() || (workspace.label != name && workspace.label != cwd_name) {
            return None;
        }
    }
    Some((root, tree.key.as_str()))
}

fn path_basename(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
}
