//! Tool-based LLM detection system
//!
//! This module implements a tool-based approach for build system detection,
//! where the LLM can call predefined tools to gather information about the
//! repository and make informed decisions.
//!
//! The tool system consists of three main components:
//! - `definitions`: Tool schemas and descriptions that the LLM can understand
//! - `executor`: Execution engine that runs tools and returns results
//! - `registry`: Central registry that manages available tools and their metadata

pub mod definitions;
pub mod executor;
pub mod registry;
