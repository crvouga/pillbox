//! Renderer-independent, fixed-step authoritative arena simulation.
mod arenas;
mod bots;
mod building;
mod content;
mod physics;
mod simulation;
pub use arenas::*;
pub use bots::*;
pub use building::*;
pub use content::*;
pub use glam::Vec3;
pub use physics::PhysicsMap;
pub use simulation::*;
