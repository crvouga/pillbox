//! Renderer-independent, fixed-step authoritative arena simulation.
mod bots;
mod content;
mod physics;
mod simulation;
pub use bots::*;
pub use content::*;
pub use glam::Vec3;
pub use physics::PhysicsMap;
pub use simulation::*;
