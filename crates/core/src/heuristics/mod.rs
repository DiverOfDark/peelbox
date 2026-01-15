// Heuristic logging and pattern matching
//
// This module provides infrastructure for logging LLM input/output pairs
// to enable future optimization through heuristic extraction.

pub mod logger;

pub use logger::HeuristicLogger;
