//! Check for Updates: asks GitHub (only when this window is opened) whether a
//! newer version has been released, and offers the download page. TidyCuts
//! never does this by itself.

use super::{footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::theme;
use egui::RichText;
use egui_phosphor::regular as ph;
use std::sync::{Arc, Mutex};

const RELEASE_API: &str =
    "https://api.github.com/repos/tidysweepsoftware-lgtm/tidycuts/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/tidysweepsoftware-lgtm/tidycuts/releases/latest";

/// What the check found.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    UpToDate,
    Newer {
        version: String,
        page: String,
        notes: String,
    },
}

/// Whether this copy comes from the Microsoft Store, which updates itself.
pub fn is_store_build() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains("\\WindowsApps\\"))
        .unwrap_or(false)
}

/// Splits "v1.2.3" or "1.2" into numbers.
fn parse_version(v: &str) -> Vec<u32> {
    v.trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map_while(|p| p.parse::<u32>().ok())
        .collect()
}

/// Whether `latest` is a higher version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (parse_version(latest), parse_version(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

/// Reads GitHub's answer for the latest release.
pub fn read_answer(json: &str, current: &str) -> Result<Answer, String> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|_| "GitHub gave an answer that could not be read.".to_string())?;
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("GitHub's answer has no version in it.")?;
    if !is_newer(tag, current) {
        return Ok(Answer::UpToDate);
    }
    let notes: String = v
        .get("body")
        .and_then(|b| b.as_str())
        .unwrap_or("")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Answer::Newer {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        page: v
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or(RELEASES_PAGE)
            .to_string(),
        notes,
    })
}

/// Asks GitHub, using the `curl` program every Windows 10/11 and Mac has.
fn fetch() -> Result<Answer, String> {
    let mut cmd = tidycuts_media::curl();
    cmd.args([
        "-s",
        "-m",
        "12",
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "User-Agent: TidyCuts",
        RELEASE_API,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd
        .output()
        .map_err(|_| "Could not start the program that checks the internet (curl).".to_string())?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err("Could not reach GitHub. Check your internet connection and try again.".into());
    }
    read_answer(
        &String::from_utf8_lossy(&out.stdout),
        env!("CARGO_PKG_VERSION"),
    )
}

type Shared = Arc<Mutex<Option<Result<Answer, String>>>>;

pub struct UpdatesDialog {
    result: Shared,
}

impl UpdatesDialog {
    pub fn new(ctx: &egui::Context) -> UpdatesDialog {
        let d = UpdatesDialog {
            result: Arc::new(Mutex::new(None)),
        };
        let (result, ctx) = (d.result.clone(), ctx.clone());
        std::thread::spawn(move || {
            *result.lock().unwrap() = Some(fetch());
            ctx.request_repaint();
        });
        d
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, _app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let state = self.result.lock().unwrap().clone();
        let mut close = false;
        let mut open_page: Option<String> = None;
        let (_, keys) = window(ctx, "updates", 460.0, |ui| {
            if header(
                ui,
                ph::ARROW_CIRCLE_UP,
                "Check for Updates",
                &format!("You have version {}", env!("CARGO_PKG_VERSION")),
            ) {
                close = true;
            }
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(20, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 10.0);
                    match &state {
                        None => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(RichText::new("Asking GitHub...").color(theme::TEXT_DIM));
                            });
                            ctx.request_repaint_after(std::time::Duration::from_millis(200));
                        }
                        Some(Err(e)) => {
                            ui.add(egui::Label::new(RichText::new(e).color(theme::WARNING)).wrap());
                        }
                        Some(Ok(Answer::UpToDate)) => {
                            ui.label(
                                RichText::new("You have the latest version.").color(theme::GOOD),
                            );
                        }
                        Some(Ok(Answer::Newer { version, notes, .. })) => {
                            ui.label(
                                RichText::new(format!("Version {version} is available."))
                                    .font(theme::heading_font(15.0))
                                    .color(theme::TEXT),
                            );
                            if !notes.is_empty() {
                                ui.add(
                                    egui::Label::new(RichText::new(notes).color(theme::TEXT_DIM))
                                        .wrap(),
                                );
                            }
                        }
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(
                                "TidyCuts only checks when you open this window. It sends nothing \
                                 about you or your projects.",
                            )
                            .size(12.0)
                            .color(theme::TEXT_FAINT),
                        )
                        .wrap(),
                    );
                });
            footer(
                ui,
                |_| {},
                |ui| {
                    if secondary(ui, "Close").clicked() {
                        close = true;
                    }
                    if let Some(Ok(Answer::Newer { page, .. })) = &state {
                        if primary(
                            ui,
                            &format!("{}  Open the download page", ph::ARROW_SQUARE_OUT),
                            true,
                        )
                        .clicked()
                        {
                            open_page = Some(page.clone());
                        }
                    }
                },
            );
        });
        if let Some(url) = open_page {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        !(close || keys.escape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(is_newer("1.2.0", "1.1.0"));
        assert!(is_newer("1.1.1", "1.1"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("v1.0.9", "1.1.0"));
    }

    #[test]
    fn the_answer_is_read_from_githubs_json() {
        let json = r###"{"tag_name":"v1.3.0","html_url":"https://example.com/r","body":"## What is new\n\n- Better sound\n- Captions"}"###;
        match read_answer(json, "1.1.0").unwrap() {
            Answer::Newer {
                version,
                page,
                notes,
            } => {
                assert_eq!(version, "1.3.0");
                assert_eq!(page, "https://example.com/r");
                assert!(notes.contains("Better sound"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(read_answer(json, "1.3.0").unwrap(), Answer::UpToDate);
        assert!(read_answer("not json", "1.0.0").is_err());
        assert!(read_answer("{}", "1.0.0").is_err());
    }
}
