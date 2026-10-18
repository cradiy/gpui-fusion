#[test]
fn frosted_glass_backdrop_shader_is_valid_wgsl() {
    let shader = gpui_effects::frosted_glass_shader();
    let source = gpui::compose_backdrop_shader_wgsl(&shader);
    let module = wgpu::naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("frosted glass backdrop shader should parse: {error}"));
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("frosted glass backdrop shader should validate: {error}"));
}

#[test]
fn built_in_effect_shaders_are_valid_wgsl() {
    for shader in [
        gpui_effects::aurora_shader(),
        gpui_effects::plasma_shader(),
        gpui_effects::color_orbs_shader(),
        gpui_effects::color_flow_shader(),
        gpui_effects::holographic_shader(),
        gpui_effects::holographic_mask_shader(),
        gpui_effects::holographic_image_shader(),
        gpui_effects::flip_shader(),
        gpui_effects::rigid_flip_shader(),
        gpui_effects::soft_flip_shader(),
        gpui_effects::spectrum_mask_shader(),
    ] {
        let source = gpui::compose_effect_shader_wgsl(&shader);
        let module =
            wgpu::naga::front::wgsl::parse_str(&source).expect("built-in effect should parse");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("built-in effect should validate");
    }
}

fn assert_effect_translates_to_msl_and_hlsl(shader: gpui::EffectShader) {
    let source = gpui::compose_effect_shader_wgsl(&shader);
    let module = naga::front::wgsl::parse_str(&source).expect("effect should parse");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("effect should validate");

    let mut resources = naga::back::msl::EntryPointResources::default();
    resources.resources.insert(
        naga::ResourceBinding {
            group: 0,
            binding: 0,
        },
        naga::back::msl::BindTarget {
            buffer: Some(0),
            ..Default::default()
        },
    );
    resources.resources.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 3,
        },
        naga::back::msl::BindTarget {
            texture: Some(1),
            ..Default::default()
        },
    );
    resources.resources.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 4,
        },
        naga::back::msl::BindTarget {
            texture: Some(2),
            ..Default::default()
        },
    );
    resources.resources.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 5,
        },
        naga::back::msl::BindTarget {
            texture: Some(3),
            ..Default::default()
        },
    );
    resources.resources.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 1,
        },
        naga::back::msl::BindTarget {
            texture: Some(0),
            ..Default::default()
        },
    );
    resources.sizes_buffer = Some(2);
    resources.resources.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 0,
        },
        naga::back::msl::BindTarget {
            buffer: Some(1),
            ..Default::default()
        },
    );
    let mut msl_options = naga::back::msl::Options {
        lang_version: (2, 0),
        fake_missing_bindings: false,
        ..Default::default()
    };
    msl_options
        .per_entry_point_map
        .insert("vs_effect".to_owned(), resources.clone());
    msl_options
        .per_entry_point_map
        .insert("fs_effect".to_owned(), resources);
    let (msl, msl_info) = naga::back::msl::write_string(
        &module,
        &info,
        &msl_options,
        &naga::back::msl::PipelineOptions::default(),
    )
    .expect("effect should translate to MSL");
    assert!(
        msl_info.entry_point_names.iter().all(Result::is_ok),
        "entry point translation failed: {:?}",
        msl_info.entry_point_names,
    );
    assert!(msl.contains("vertex"), "{msl}");
    assert!(msl.contains("fragment"), "{msl}");
    if shader.uses_image() || shader.is_mask() {
        assert!(msl.contains("texture2d"), "{msl}");
    }
    assert_eq!(msl_info.entry_point_names.len(), 2);
    let mut hlsl_options = naga::back::hlsl::Options {
        shader_model: naga::back::hlsl::ShaderModel::V5_0,
        fake_missing_bindings: false,
        ..Default::default()
    };
    hlsl_options.binding_map.insert(
        naga::ResourceBinding {
            group: 0,
            binding: 0,
        },
        naga::back::hlsl::BindTarget {
            space: 0,
            register: 0,
            ..Default::default()
        },
    );
    hlsl_options.binding_map.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 3,
        },
        naga::back::hlsl::BindTarget {
            space: 0,
            register: 2,
            ..Default::default()
        },
    );
    hlsl_options.binding_map.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 4,
        },
        naga::back::hlsl::BindTarget {
            space: 0,
            register: 3,
            ..Default::default()
        },
    );
    hlsl_options.binding_map.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 5,
        },
        naga::back::hlsl::BindTarget {
            space: 0,
            register: 4,
            ..Default::default()
        },
    );
    hlsl_options.binding_map.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 1,
        },
        naga::back::hlsl::BindTarget {
            space: 0,
            register: 0,
            ..Default::default()
        },
    );
    hlsl_options.binding_map.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 0,
        },
        naga::back::hlsl::BindTarget {
            space: 0,
            register: 1,
            ..Default::default()
        },
    );
    let mut hlsl = String::new();
    naga::back::hlsl::Writer::new(
        &mut hlsl,
        &hlsl_options,
        &naga::back::hlsl::PipelineOptions::default(),
    )
    .write(&module, &info, None)
    .expect("effect should translate to HLSL");
    assert!(hlsl.contains("vs_effect"));
    assert!(hlsl.contains("fs_effect"));
    if shader.uses_image() || shader.is_mask() {
        assert!(hlsl.contains("Texture2D"), "{hlsl}");
    }
}

#[test]
fn built_in_effect_translates_to_msl_and_hlsl() {
    for shader in [
        gpui_effects::color_flow_shader(),
        gpui_effects::holographic_shader(),
        gpui_effects::holographic_mask_shader(),
        gpui_effects::holographic_image_shader(),
        gpui_effects::flip_shader(),
        gpui_effects::spectrum_mask_shader(),
    ] {
        assert_effect_translates_to_msl_and_hlsl(shader);
    }
}
