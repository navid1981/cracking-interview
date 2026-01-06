// AI service module
// Will implement Gemini and Claude services here

pub mod gemini;
pub mod claude;

// Re-export for convenience
pub use gemini::*;
pub use claude::*;
