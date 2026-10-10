//! A small Model Context Protocol server, so Claude can drive the editor.
//!
//! [`protocol`] speaks MCP (JSON-RPC 2.0); [`http`] carries it over a local
//! HTTP port protected by a secret token. The editor supplies a [`Host`]
//! that says which tools exist and what they do.

pub mod http;
pub mod protocol;

pub use http::{new_token, Server, ENDPOINT};
pub use protocol::{base64, Content, Host, Tool, ToolResult};
