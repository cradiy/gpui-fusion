//! Particle systems and fluid simulation.

mod fluid;
mod particles;

pub use fluid::{Fluid, FluidOptions, FluidSplat, fluid};
pub use particles::{
    ParticleMask, ParticlePhysics, ParticleSpawn, Particles, particles, subtree_particles,
};
