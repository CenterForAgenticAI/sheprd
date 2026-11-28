//! Repository lookups (the pull request section, the new worktree dialog's
//! branch and GitHub tabs) for a main checkout whose shell left the
//! repository Herdr keeps for it, and for a linked checkout routed through
//! such a main checkout.

use super::*;
use crate::pull_request::{Input, Origin};

const MOVED: &str = "/fixture/wgsextract-cli";

fn open(view: &mut SheprdWindow, workspace: usize, linked_route: bool) {
    view.live.status = crate::state::ConnectionStatus::Connected;
    let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
    snapshot.workspaces = sidebar::layout_tests::snapshot(7).workspaces;
    snapshot.workspaces[3].new_workspace_cwd = MOVED.into();
    // Its label follows the shell into the other repository.
    snapshot.workspaces[3].label = "wgsextract-cli".into();
    snapshot.workspaces[3].branch = Some("main".into());
    let target = if linked_route {
        WorkspaceTarget::for_new_worktree(snapshot, &snapshot.workspaces[workspace]).unwrap()
    } else {
        WorkspaceTarget::new(snapshot, &snapshot.workspaces[workspace])
    };
    view.menu.target = Some(target);
}

fn move_again(view: &mut SheprdWindow) {
    let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
    snapshot.workspaces[3].new_workspace_cwd = "/fixture/third".into();
}

/// A moved main checkout is looked up by the directory it is in, and stops
/// being the menu's workspace once it moves again, rather than keeping the
/// previous repository's pull request.
#[gpui::test]
#[cfg(unix)]
fn a_moved_main_checkout_is_looked_up_where_it_is_until_it_moves_again(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(sidebar::layout_tests::fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            open(view, 3, false);
            assert_eq!(
                view.menu_target_input(&Origin::Local).unwrap(),
                Input {
                    checkout: Some(MOVED.into()),
                    repo_key: None,
                    branch: "main".into(),
                }
            );
            move_again(view);
            assert!(view.menu_target_workspace().is_none());
            assert!(matches!(
                view.menu_target_input(&Origin::Local),
                Err(crate::Error::StaleWorkspace)
            ));
        })
    });
}

/// A linked checkout routed through a moved main checkout is looked up by
/// its repository's Git directory and its own branch, never the moved main
/// checkout's branch of another repository. A move of the main checkout to
/// yet another directory leaves that unchanged.
#[gpui::test]
fn a_linked_route_reads_its_repository_with_its_own_branch(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(sidebar::layout_tests::fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            open(view, 4, true);
            let expected = Input {
                checkout: None,
                repo_key: Some(sidebar::layout_tests::REPO_KEY.into()),
                branch: "worktree/sidebar-child".into(),
            };
            assert_eq!(view.menu_target_input(&Origin::Local).unwrap(), expected);
            move_again(view);
            assert_eq!(view.menu_target_input(&Origin::Local).unwrap(), expected);
            // Once the main checkout is back in its repository, the route no
            // longer stands.
            let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
            snapshot.workspaces[3].new_workspace_cwd = sidebar::layout_tests::REPO_KEY
                .trim_end_matches("/.git")
                .into();
            assert!(view.menu_target_workspace().is_none());
        })
    });
}

fn attach_root_pane(
    snapshot: &mut sheprd_client::protocol::ClientShellSnapshot,
    workspace_id: &str,
    cwd: &str,
) {
    let tab_id = format!("{workspace_id}:t1");
    snapshot.tabs.retain(|tab| tab.workspace_id != workspace_id);
    snapshot
        .panes
        .retain(|pane| pane.workspace_id != workspace_id);
    snapshot.tabs.push(sheprd_client::protocol::ClientShellTab {
        tab_id: tab_id.clone(),
        workspace_id: workspace_id.into(),
        number: 1,
        label: "1".into(),
        custom_label: false,
        zoomed: false,
        focused: false,
        agent_status: sheprd_client::protocol::AgentStatus::Idle,
    });
    snapshot
        .panes
        .push(sheprd_client::protocol::ClientShellPane {
            pane_id: format!("{workspace_id}:p1"),
            workspace_id: workspace_id.into(),
            tab_id,
            label: None,
            cwd: Some(cwd.into()),
            foreground_cwd: None,
            focused: false,
            right_click_passthrough: false,
        });
}

/// A main checkout whose focused pane wandered outside the repository while
/// its first tab's root pane stayed in it schedules, retains, and presents the
/// same `repo_key`-based PR lookup, so the menu shows its PR instead of staying
/// on "Checking". Moving the root shell into another repository — even one with
/// the same folder name — invalidates the open menu target and switches future
/// lookups to the new directory.
#[gpui::test]
#[cfg(unix)]
fn a_wandered_pane_schedules_retains_and_presents_its_repository_pr(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(sidebar::layout_tests::fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.live.local_daemon_peer = true;
            let root = sidebar::layout_tests::REPO_KEY.trim_end_matches("/.git");
            {
                let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
                snapshot.workspaces = sidebar::layout_tests::snapshot(7).workspaces;
                snapshot.focused_workspace_id = Some("w3".into());
                snapshot.workspaces[3].new_workspace_cwd = "/Users/me".into();
                attach_root_pane(snapshot, "w3", root);
            }
            let expected = Input {
                checkout: None,
                repo_key: Some(sidebar::layout_tests::REPO_KEY.into()),
                branch: "develop".into(),
            };
            assert_eq!(view.git_input(), Some(expected.clone()));
            // Background scheduling uses the same `repo_key` input on local
            // and SSH hosts alike.
            {
                let snapshot = view.live.snapshot.as_ref().unwrap();
                assert_eq!(
                    super::super::pr::workspace_pr_inputs(snapshot, &Origin::Local, Some("w3"), 0)
                        .next(),
                    Some(expected.clone())
                );
                assert_eq!(
                    super::super::pr::workspace_pr_inputs(
                        snapshot,
                        &Origin::Ssh("device".into()),
                        Some("w3"),
                        0,
                    )
                    .next(),
                    Some(expected.clone())
                );
            }
            view.seed_pr(
                expected.clone(),
                crate::pull_request::fixture().unwrap(),
                std::time::Instant::now(),
            );
            view.open_workspace_menu("w3", Default::default(), window, cx);
            assert_eq!(view.menu_target_input(&Origin::Local).unwrap(), expected);
            assert_eq!(view.menu.pr.value.as_ref().unwrap().number, 8);
            assert!(!view.menu.pr.loading);

            // A fresh snapshot Arc exercises `pr_cache.retain` in
            // `update_workspace_pr`: the `repo_key` entry stays cached and
            // presented rather than being evicted back to "Checking".
            std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap()).revision += 1;
            view.update_workspace_pr();
            view.refresh_workspace_pr();
            assert_eq!(view.menu.pr.value.as_ref().unwrap().number, 8);
            assert!(!view.menu.pr.loading);

            // Moving the root shell into another repository with the same
            // folder name invalidates the open menu target and switches
            // scheduling to that directory.
            {
                let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
                snapshot.workspaces[3].new_workspace_cwd = "/other/agent-launcher".into();
                snapshot.workspaces[3].label = "agent-launcher".into();
                attach_root_pane(snapshot, "w3", "/other/agent-launcher");
            }
            assert!(view.menu_target_workspace().is_none());
            assert!(matches!(
                view.menu_target_input(&Origin::Local),
                Err(crate::Error::StaleWorkspace)
            ));
            let snapshot = view.live.snapshot.as_ref().unwrap();
            assert_eq!(
                super::super::pr::workspace_pr_inputs(snapshot, &Origin::Local, Some("w3"), 0)
                    .next(),
                Some(Input {
                    checkout: Some("/other/agent-launcher".into()),
                    repo_key: None,
                    branch: "develop".into(),
                })
            );
        })
    });
}
