//! Lets Claude drive the editor. This is the glue: the server (the
//! `tidycuts-mcp` crate) answers on its own threads, but the project belongs
//! to the window, so every tool call is sent here as a [`Job`], run on the
//! window's thread between frames (`mcp_tools.rs`), and the answer goes back
//! to the waiting connection.

use crate::prefs::Prefs;
use serde_json::Value;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;
use tidycuts_mcp::{Host, Server, Tool, ToolResult};

/// How long a tool call waits for the window before giving up.
const ANSWER_WITHIN: Duration = Duration::from_secs(90);
/// Most jobs run in one frame, so a flood can't freeze the window.
const JOBS_PER_FRAME: usize = 8;

/// One tool call waiting to run.
pub struct Job {
    pub tool: String,
    pub args: Value,
    pub reply: Sender<ToolResult>,
}

struct AppHost {
    tx: Sender<Job>,
    ctx: egui::Context,
}

impl Host for AppHost {
    fn instructions(&self) -> String {
        crate::mcp_tools::INSTRUCTIONS.to_string()
    }

    fn tools(&self) -> Vec<Tool> {
        crate::mcp_tools::tool_list()
    }

    fn call(&self, name: &str, args: &Value) -> ToolResult {
        let (reply, answer) = channel();
        let job = Job {
            tool: name.to_string(),
            args: args.clone(),
            reply,
        };
        if self.tx.send(job).is_err() {
            return ToolResult::error("The editor is closing.");
        }
        // The window only runs when something asks it to.
        self.ctx.request_repaint();
        match answer.recv_timeout(ANSWER_WITHIN) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => ToolResult::error(
                "The editor did not answer in time. Is the window minimized or busy? \
                 Bring it to the front and try again.",
            ),
            Err(RecvTimeoutError::Disconnected) => ToolResult::error("The editor is closing."),
        }
    }
}

/// The server and the queue of calls for the window.
pub struct McpState {
    jobs_tx: Sender<Job>,
    jobs_rx: Receiver<Job>,
    server: Option<Server>,
    /// What the running server was started with, to notice changes.
    started_with: Option<(u16, String)>,
    /// Why the server could not start, if it could not.
    pub error: Option<String>,
}

impl Default for McpState {
    fn default() -> Self {
        let (jobs_tx, jobs_rx) = channel();
        McpState {
            jobs_tx,
            jobs_rx,
            server: None,
            started_with: None,
            error: None,
        }
    }
}

impl McpState {
    /// Starts, stops or restarts the server so it matches the preferences.
    pub fn sync(&mut self, prefs: &Prefs, ctx: &egui::Context) {
        if !prefs.mcp_enabled || prefs.mcp_token.is_empty() {
            self.server = None;
            self.started_with = None;
            self.error = None;
            return;
        }
        let want = (prefs.mcp_port, prefs.mcp_token.clone());
        if self.server.is_some() && self.started_with.as_ref() == Some(&want) {
            return;
        }
        self.server = None;
        let host = Arc::new(AppHost {
            tx: self.jobs_tx.clone(),
            ctx: ctx.clone(),
        });
        // The usual port first, so a client's saved setup keeps working;
        // any free one if something else has it.
        let started = Server::start(host.clone(), want.1.clone(), want.0)
            .or_else(|_| Server::start(host, want.1.clone(), 0));
        match started {
            Ok(s) => {
                log::info!("AI assistant connection listening on {}", s.url());
                self.started_with = Some((want.0, want.1));
                self.server = Some(s);
                self.error = None;
            }
            Err(e) => {
                self.started_with = None;
                self.error = Some(format!("Could not start the connection: {e}"));
            }
        }
    }

    pub fn url(&self) -> Option<String> {
        self.server.as_ref().map(Server::url)
    }

    /// Calls waiting for the window (a few at a time).
    pub fn take_jobs(&self) -> Vec<Job> {
        self.jobs_rx.try_iter().take(JOBS_PER_FRAME).collect()
    }
}
