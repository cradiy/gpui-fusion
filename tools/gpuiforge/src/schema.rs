use crate::config::Platform;
use anyhow::Result;
use serde_json::{Value, json};

pub fn text(recipe: bool) -> Result<String> {
    let mut platform = serde_json::to_value(schemars::schema_for!(Platform))?;
    non_null_options(&mut platform);
    let properties = platform["properties"].as_object_mut().unwrap();
    properties.get_mut("inset-handling").unwrap()["default"] = "application".into();
    properties.get_mut("abis").unwrap()["items"] = json!({"oneOf":[
        {"type":"string", "const":"arm64-v8a", "description":"64-bit ARM Android devices. Uses Rust target aarch64-linux-android; commonly used for physical phones and ARM emulators."},
        {"type":"string", "const":"x86_64", "description":"64-bit x86 Android devices or emulators. Uses Rust target x86_64-linux-android; select it for an x86_64 emulator."}
    ]});
    properties.get_mut("abis").unwrap()["minItems"] = 1.into();
    properties
        .get_mut("abis")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("default");
    properties.get_mut("application-id").unwrap()["pattern"] =
        "^[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z][A-Za-z0-9_]*)+$".into();
    for (field, examples) in [
        ("icon", json!(["assets/app.png"])),
        ("notification-icon", json!(["assets/notification.xml"])),
        ("application-id", json!(["com.example.app"])),
        (
            "features",
            json!([["files", "sharing"], ["media", "media-notifications"]]),
        ),
        (
            "permissions",
            json!([[
                "android.permission.INTERNET",
                "android.permission.POST_NOTIFICATIONS"
            ]]),
        ),
        ("url-schemes", json!([["myapp"]])),
        ("share-mime-types", json!([["text/plain", "image/*"]])),
    ] {
        properties.get_mut(field).unwrap()["examples"] = examples;
    }
    properties.get_mut("variables").unwrap()["properties"] = json!({
        "min_sdk": {
            "type":"string", "default":"26",
            "description":"Minimum Android API level for the bundled host and NDK build. Defaults to 26 and must be at least 26. Keep this as a string. Raising it excludes older Android devices."
        },
        "target_sdk": {
            "type":"string", "default":"36",
            "description":"Target Android API level declared by the bundled application. Defaults to 36. Keep this as a string. This controls Android compatibility behavior; it does not change the template's compileSdk or install SDK packages."
        },
        "version_code": {
            "type":"string", "default":"1",
            "description":"Integer Android versionCode expressed as a string. Defaults to 1. Increase it for each application release distributed as an update."
        },
        "version_name": {
            "type":"string", "default":"0.1.0",
            "description":"User-visible Android versionName. Defaults to 0.1.0. Independent of the Cargo package version; change it explicitly when releasing."
        },
        "native_library": {
            "type":"string", "examples":["my_app"],
            "description":"Rust library name loaded by the bundled Android Activity and used by the packaging helper, without lib prefix or .so suffix. Defaults to the application's Cargo package name with hyphens replaced by underscores. Usually leave this unset."
        }
    });
    properties.get_mut("paths").unwrap()["additionalProperties"]["description"] =
        "An existing file or directory relative to the declaring JSON file, or an absolute path. Reference it as {{path.<key>}}.".into();
    properties.get_mut("variables").unwrap()["additionalProperties"]["description"] =
        "Custom string value available as {{var.<key>}}. Additional keys are allowed.".into();
    if recipe {
        platform["title"] = "GPUiForge platform recipe".into();
        platform["properties"]
            .as_object_mut()
            .unwrap()
            .remove("recipe");
        return Ok(serde_json::to_string_pretty(&platform)?);
    }
    let mut definitions = platform
        .as_object_mut()
        .unwrap()
        .remove("$defs")
        .unwrap_or(json!({}));
    platform.as_object_mut().unwrap().remove("$schema");
    platform.as_object_mut().unwrap().remove("title");
    let mut commands = platform.clone();
    commands["properties"]["variables"]
        .as_object_mut()
        .unwrap()
        .remove("properties");
    commands["properties"]["variables"]["description"] =
        "Custom string values exposed as {{var.<name>}} in templates and process steps. Application values override individual recipe keys.".into();
    for name in [
        "features",
        "icon",
        "notification-icon",
        "inset-handling",
        "permissions",
        "signing",
        "url-schemes",
        "share-mime-types",
        "abis",
        "application-id",
        "activity",
    ] {
        commands["properties"].as_object_mut().unwrap().remove(name);
    }
    platform["anyOf"] = json!([{"required":["application-id"]},{"required":["recipe"]}]);
    definitions["AndroidPlatform"] = platform;
    definitions["CommandPlatform"] = commands;
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "GPUiForge application configuration",
        "type": "object",
        "additionalProperties": false,
        "required": ["app", "platforms"],
        "properties": {
            "$schema": {"type":"string", "examples":["./gpuiforge.schema.json"], "description":"Schema reference used by your editor for completion, hover help and validation. Relative references resolve from this JSON file. gpuiforge init writes a local schema for offline use; GPUiForge does not fetch schema URLs."},
            "app": {"type":"object", "description":"Application-wide display settings shared by platform templates. Platform identities and build commands belong under platforms.", "additionalProperties":false, "required":["name"], "properties":{
                "name":{"type":"string", "pattern":"\\S", "examples":["My App"], "description":"Nonempty display name exposed as {{app.name}} and used as the bundled Android application's label. Does not rename the Cargo package, Android application-id or executable."}
            }},
            "platforms": {"type":"object", "description":"Configured build/run targets. At least one is required. GPUiForge offers these entries when no platform is specified; omitting an entry removes it from the selection menu.", "additionalProperties":false, "minProperties":1, "properties": {
                "android":{"$ref":"#/$defs/AndroidPlatform", "description":"Android application packaging and device launch. Uses bundled Kotlin/Gradle templates unless a custom recipe or template is selected. Configure modules, ABI selection, permissions, icons and release signing here."},
                "desktop":{"$ref":"#/$defs/CommandPlatform", "description":"Commands for the current desktop host (Linux, macOS or Windows). No native project is generated unless a template is configured. Run steps must build the application if necessary, for example cargo run."},
                "web":{"$ref":"#/$defs/CommandPlatform", "description":"Commands for Web builds and development servers. No bundled Web recipe or server is supplied; configure tools such as your existing WASM build command in build and run."}
            }}
        },
        "$defs": definitions
    });
    Ok(serde_json::to_string_pretty(&schema)?)
}

fn non_null_options(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.get("default") == Some(&Value::Null) {
                object.remove("default");
            }
            if let Some(Value::Array(types)) = object.get_mut("type") {
                types.retain(|ty| ty != "null");
                let single = (types.len() == 1).then(|| types[0].clone());
                if let Some(single) = single {
                    object.insert("type".into(), single);
                }
            }
            if let Some(Value::Array(options)) = object.get_mut("anyOf") {
                options.retain(|option| option.get("type") != Some(&json!("null")));
                if options.len() == 1 {
                    let only = options[0].as_object().unwrap().clone();
                    object.remove("anyOf");
                    object.extend(only);
                }
            }
            for value in object.values_mut() {
                non_null_options(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                non_null_options(value);
            }
        }
        _ => {}
    }
}
