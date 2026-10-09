use crate::WgpuContext;

#[derive(Clone, Copy)]
pub(super) struct SimulationCapabilities {
    pub particles: bool,
    pub fluid: bool,
}

impl SimulationCapabilities {
    pub fn query(context: &WgpuContext) -> Self {
        Self::from_limits(
            &context.device.limits(),
            context.adapter.get_downlevel_capabilities().flags,
        )
    }

    fn from_limits(limits: &wgpu::Limits, flags: wgpu::DownlevelFlags) -> Self {
        let compute = flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
            && limits.max_bindings_per_bind_group >= 5
            && limits.max_storage_buffers_per_shader_stage >= 4
            && limits.max_buffers_and_acceleration_structures_per_shader_stage >= 5
            && limits.max_uniform_buffers_per_shader_stage >= 1
            && limits.max_uniform_buffer_binding_size >= 128
            && limits.max_compute_invocations_per_workgroup >= 64
            && limits.max_compute_workgroup_size_x >= 8
            && limits.max_compute_workgroup_size_y >= 8
            && limits.max_compute_workgroup_size_z >= 1;
        let buffer_fits = |bytes: u64| {
            limits.max_buffer_size >= bytes && limits.max_storage_buffer_binding_size >= bytes
        };
        // Particle state uses 48 bytes per slot; mask candidates use a 16-byte
        // header and 32 bytes per sample. Fluid fields contain one vec4 per cell.
        let particle_bytes = (u64::from(gpui::MAX_GPU_PARTICLES) * 48)
            .max(16 + 32 * u64::from(super::particles::MAX_MASK_SAMPLES));
        Self {
            particles: compute
                && flags.contains(wgpu::DownlevelFlags::VERTEX_STORAGE)
                && limits.max_bind_groups >= 2
                && limits.max_sampled_textures_per_shader_stage >= 1
                && limits.max_compute_workgroup_size_x >= 64
                && limits.max_compute_workgroups_per_dimension
                    >= gpui::MAX_GPU_PARTICLES
                        .div_ceil(64)
                        .max(limits.max_texture_dimension_2d.div_ceil(8))
                && buffer_fits(particle_bytes),
            fluid: compute
                && flags.contains(wgpu::DownlevelFlags::FRAGMENT_STORAGE)
                && limits.max_bind_groups >= 1
                && limits.max_compute_workgroups_per_dimension >= 64
                && buffer_fits(512 * 512 * 16),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_requires_enabled_compute_storage_and_stage_support() {
        let flags = wgpu::DownlevelFlags::COMPUTE_SHADERS
            | wgpu::DownlevelFlags::VERTEX_STORAGE
            | wgpu::DownlevelFlags::FRAGMENT_STORAGE;
        let limits = wgpu::Limits::default();
        let supported = SimulationCapabilities::from_limits(&limits, flags);
        assert!(supported.particles && supported.fluid);
        for flag in [
            wgpu::DownlevelFlags::COMPUTE_SHADERS,
            wgpu::DownlevelFlags::VERTEX_STORAGE,
            wgpu::DownlevelFlags::FRAGMENT_STORAGE,
        ] {
            let result = SimulationCapabilities::from_limits(&limits, flags - flag);
            assert_eq!(
                result.particles,
                flag == wgpu::DownlevelFlags::FRAGMENT_STORAGE
            );
            assert_eq!(result.fluid, flag == wgpu::DownlevelFlags::VERTEX_STORAGE);
        }
        for limits in [
            wgpu::Limits {
                max_storage_buffers_per_shader_stage: 3,
                ..limits.clone()
            },
            wgpu::Limits {
                max_compute_invocations_per_workgroup: 32,
                ..limits.clone()
            },
            wgpu::Limits {
                max_storage_buffer_binding_size: 1024,
                ..limits
            },
        ] {
            let result = SimulationCapabilities::from_limits(&limits, flags);
            assert!(!result.particles && !result.fluid);
        }
    }
}
