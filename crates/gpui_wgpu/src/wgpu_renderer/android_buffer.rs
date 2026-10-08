use ash::vk;
use gpui_util::android::hardware_buffer::HardwareBufferFrame;
use std::{
    ffi::CStr,
    os::fd::{AsRawFd, IntoRawFd},
    sync::Arc,
};

const EXTENSIONS: [&CStr; 3] = [
    ash::android::external_memory_android_hardware_buffer::NAME,
    ash::ext::queue_family_foreign::NAME,
    ash::khr::external_semaphore_fd::NAME,
];

pub(crate) fn create_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<(wgpu::Device, wgpu::Queue)> {
    let result =
        (|| -> anyhow::Result<_> {
            let hal = unsafe { adapter.as_hal::<wgpu::hal::vulkan::Api>() }
                .ok_or_else(|| anyhow::anyhow!("not Vulkan"))?;
            let properties = unsafe {
                hal.shared_instance()
                    .raw_instance()
                    .get_physical_device_properties(hal.raw_physical_device())
            };
            anyhow::ensure!(
                properties.api_version >= vk::API_VERSION_1_1,
                "hardware buffer import requires Vulkan 1.1"
            );
            let extensions = unsafe {
                hal.shared_instance()
                    .raw_instance()
                    .enumerate_device_extension_properties(hal.raw_physical_device())
            }?;
            anyhow::ensure!(
                EXTENSIONS
                    .iter()
                    .all(|extension| extensions.iter().any(|available| unsafe {
                        CStr::from_ptr(available.extension_name.as_ptr())
                    } == *extension)),
                "Android external memory extensions unavailable"
            );
            let mut sync = vk::ExternalSemaphoreProperties::default();
            unsafe {
                hal.shared_instance()
                    .raw_instance()
                    .get_physical_device_external_semaphore_properties(
                        hal.raw_physical_device(),
                        &vk::PhysicalDeviceExternalSemaphoreInfo::default()
                            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD),
                        &mut sync,
                    );
            }
            anyhow::ensure!(
                sync.external_semaphore_features
                    .contains(vk::ExternalSemaphoreFeatureFlags::IMPORTABLE),
                "native GPU fence import unavailable"
            );
            // Vulkan 1.1 supplies the AHB
            // extension's external-memory, dedicated-allocation and YCbCr dependencies.
            let opened = unsafe {
                hal.open_with_callback(
                    descriptor.required_features,
                    &descriptor.required_limits,
                    &descriptor.memory_hints,
                    Some(Box::new(|args| {
                        for extension in EXTENSIONS {
                            if !args.extensions.contains(&extension) {
                                args.extensions.push(extension);
                            }
                        }
                    })),
                )
            }?;
            drop(hal);
            Ok(unsafe {
                adapter.create_device_from_hal::<wgpu::hal::vulkan::Api>(opened, descriptor)
            }?)
        })();
    match result {
        Ok(device) => Some(device),
        Err(error) => {
            log::info!("Android GPU frame import unavailable: {error:#}");
            None
        }
    }
}

pub(crate) fn supported(device: &wgpu::Device) -> bool {
    unsafe { device.as_hal::<wgpu::hal::vulkan::Api>() }.is_some_and(|device| {
        EXTENSIONS
            .iter()
            .all(|extension| device.enabled_device_extensions().contains(extension))
    })
}

struct Imported {
    device: ash::Device,
    image: vk::Image,
    memory: vk::DeviceMemory,
    semaphore: vk::Semaphore,
    // The producer cannot reuse its pool slot until wgpu retires this texture.
    _frame: Arc<HardwareBufferFrame>,
}

impl Drop for Imported {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_semaphore(self.semaphore, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

pub(crate) fn copy(
    instance: &wgpu::Instance,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &Arc<HardwareBufferFrame>,
    target: &wgpu::Texture,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        supported(device),
        "renderer does not support hardware buffers"
    );
    let instance = unsafe { instance.as_hal::<wgpu::hal::vulkan::Api>() }.unwrap();
    let hal = unsafe { device.as_hal::<wgpu::hal::vulkan::Api>() }.unwrap();
    let raw = hal.raw_device();
    let extension = ash::android::external_memory_android_hardware_buffer::Device::new(
        instance.shared_instance().raw_instance(),
        raw,
    );
    let buffer = frame.buffer();
    let mut format = vk::AndroidHardwareBufferFormatPropertiesANDROID::default();
    let mut properties =
        vk::AndroidHardwareBufferPropertiesANDROID::default().push_next(&mut format);
    unsafe {
        extension.get_android_hardware_buffer_properties(buffer.as_ptr().cast(), &mut properties)
    }?;
    let allocation_size = properties.allocation_size;
    let memory_type_bits = properties.memory_type_bits;
    anyhow::ensure!(
        format.format == vk::Format::R8G8B8A8_UNORM,
        "hardware buffer is not Vulkan RGBA8"
    );
    anyhow::ensure!(
        format
            .format_features
            .contains(vk::FormatFeatureFlags::TRANSFER_SRC),
        "hardware buffer does not support GPU copies"
    );
    let memory_type_index = (0..32)
        .find(|index| memory_type_bits & (1 << index) != 0)
        .ok_or_else(|| anyhow::anyhow!("hardware buffer has no importable memory type"))?;
    let mut imported = Imported {
        device: raw.clone(),
        image: vk::Image::null(),
        memory: vk::DeviceMemory::null(),
        semaphore: vk::Semaphore::null(),
        _frame: frame.clone(),
    };
    let mut external = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID);
    let extent = vk::Extent3D {
        width: buffer.width(),
        height: buffer.height(),
        depth: 1,
    };
    imported.image = unsafe {
        raw.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format.format)
                .extent(extent)
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::SAMPLED)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .push_next(&mut external),
            None,
        )
    }?;
    let mut ahb =
        vk::ImportAndroidHardwareBufferInfoANDROID::default().buffer(buffer.as_ptr().cast());
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(imported.image);
    // AHB memory requirements may only be queried after binding. Its allocation
    // size and memory types come from vkGetAndroidHardwareBufferPropertiesANDROID.
    imported.memory = unsafe {
        raw.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(allocation_size)
                .memory_type_index(memory_type_index)
                .push_next(&mut ahb)
                .push_next(&mut dedicated),
            None,
        )
    }?;
    unsafe { raw.bind_image_memory(imported.image, imported.memory, 0) }?;
    imported.semaphore =
        unsafe { raw.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }?;
    let fence = frame.acquire_fence().try_clone()?;
    let semaphore_fd = ash::khr::external_semaphore_fd::Device::new(
        instance.shared_instance().raw_instance(),
        raw,
    );
    unsafe {
        semaphore_fd.import_semaphore_fd(
            &vk::ImportSemaphoreFdInfoKHR::default()
                .semaphore(imported.semaphore)
                .flags(vk::SemaphoreImportFlags::TEMPORARY)
                .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD)
                .fd(fence.as_raw_fd()),
        )
    }?;
    let _ = fence.into_raw_fd(); // Vulkan owns the successfully imported fd.

    let image = imported.image;
    let semaphore = imported.semaphore;
    let family = hal.queue_family_index();
    let raw = raw.clone();
    let size = wgpu::Extent3d {
        width: extent.width,
        height: extent.height,
        depth_or_array_layers: 1,
    };
    let hal_texture = unsafe {
        hal.texture_from_raw(
            image,
            &wgpu::hal::TextureDescriptor {
                label: Some("android_video_buffer"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::wgt::TextureUses::COPY_SRC,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            Some(Box::new(move || drop(imported))),
            wgpu::hal::vulkan::TextureMemory::External,
        )
    };
    drop(hal);
    let source = unsafe {
        device.create_texture_from_hal::<wgpu::hal::vulkan::Api>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("android_video_buffer"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::wgt::TextureUses::COPY_SRC,
        )
    };
    let range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .level_count(1)
        .layer_count(1);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("android_video_copy"),
    });
    unsafe {
        encoder.as_hal_mut::<wgpu::hal::vulkan::Api, _, _>(|encoder| {
            raw.cmd_pipeline_barrier(
                encoder.unwrap().raw_handle(),
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .image(image)
                    .subresource_range(range)
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .dst_queue_family_index(family)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_READ)],
            );
        });
    }
    let acquire = encoder.finish();
    // wgpu 30 does not mix native and wgpu recording in one encoder.
    // Submit all three buffers together so their resource lease covers release.
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("android_video_copy"),
    });
    encoder.copy_texture_to_texture(source.as_image_copy(), target.as_image_copy(), size);
    let copy = encoder.finish();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("android_video_release"),
    });
    unsafe {
        encoder.as_hal_mut::<wgpu::hal::vulkan::Api, _, _>(|encoder| {
            raw.cmd_pipeline_barrier(
                encoder.unwrap().raw_handle(),
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .image(image)
                    .subresource_range(range)
                    .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_queue_family_index(family)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .src_access_mask(vk::AccessFlags::TRANSFER_READ)],
            );
        });
    }
    let release = encoder.finish();
    unsafe { queue.as_hal::<wgpu::hal::vulkan::Api>() }
        .unwrap()
        .add_wait_semaphore(semaphore, None, vk::PipelineStageFlags::TRANSFER);
    queue.submit([acquire, copy, release]);
    queue.on_submitted_work_done(move || drop(source));
    Ok(())
}
