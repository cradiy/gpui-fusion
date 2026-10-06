use anyhow::{Context, Result, ensure};
use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    process::Command,
};

pub struct Device {
    serial: String,
    state: String,
    model: String,
    abis: Vec<String>,
    problem: Option<String>,
}

impl Device {
    fn abi<'a>(&'a self, enabled: &[String]) -> Option<&'a str> {
        if self.state != "device" {
            return None;
        }
        self.abis
            .iter()
            .find(|abi| enabled.contains(abi))
            .map(String::as_str)
    }

    pub fn label(&self) -> String {
        let status = match self.state.as_str() {
            "device" => "ready",
            "unauthorized" => "unauthorized (allow USB debugging on the device)",
            "offline" => "offline (reconnect the device or restart the emulator)",
            "no permissions" => "no permissions (check USB device access permissions)",
            other => other,
        };
        let architecture = if self.abis.is_empty() {
            "unknown".into()
        } else {
            self.abis.join(", ")
        };
        let mut label = format!(
            "{} | {} | {} | {status}",
            self.serial, self.model, architecture
        );
        if let Some(problem) = &self.problem {
            label.push_str(&format!(" | ABI query failed: {problem}"));
        }
        label
    }
}

pub fn adb() -> PathBuf {
    std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(|sdk| {
            PathBuf::from(sdk)
                .join("platform-tools")
                .join(if cfg!(windows) { "adb.exe" } else { "adb" })
        })
        .unwrap_or_else(|| PathBuf::from("adb"))
}

fn output(args: &[&str]) -> Result<String> {
    let output = Command::new(adb())
        .args(args)
        .output()
        .context("starting adb; set ANDROID_HOME or add adb to PATH")?;
    ensure!(
        output.status.success(),
        "adb failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

pub fn list() -> Result<Vec<Device>> {
    let mut devices = Vec::new();
    for line in output(&["devices", "-l"])?.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("List of devices") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(serial) = fields.next() else {
            continue;
        };
        let Some(state) = fields.next() else {
            continue;
        };
        let state = if state == "no" && fields.next() == Some("permissions") {
            "no permissions"
        } else {
            state
        };
        let model = fields
            .find_map(|field| field.strip_prefix("model:"))
            .map(|name| name.replace('_', " "))
            .unwrap_or_else(|| "Unknown model".into());
        let mut device = Device {
            serial: serial.into(),
            state: state.into(),
            model,
            abis: Vec::new(),
            problem: None,
        };
        if state == "device" {
            match output(&["-s", serial, "shell", "getprop", "ro.product.cpu.abilist"]) {
                Ok(abis) => {
                    device.abis = abis
                        .trim()
                        .split(',')
                        .map(str::trim)
                        .filter(|abi| !abi.is_empty())
                        .map(str::to_owned)
                        .collect()
                }
                Err(error) => device.problem = Some(format!("{error:#}")),
            }
        }
        devices.push(device);
    }
    Ok(devices)
}

pub fn print() -> Result<()> {
    let devices = list()?;
    if devices.is_empty() {
        println!("No Android devices found. Connect a device or start an emulator.");
    } else {
        println!("SERIAL | MODEL | ABI | STATUS");
        for device in devices {
            println!("{}", device.label());
        }
    }
    Ok(())
}

pub fn select(enabled: &[String], requested: Option<&str>) -> Result<(String, String)> {
    let devices = list()?;
    if let Some(serial) = requested {
        let device = devices
            .iter()
            .find(|device| device.serial == serial)
            .with_context(|| {
                format!("device {serial} is not connected; use `gpuiforge devices` to list devices")
            })?;
        ensure!(
            device.state == "device",
            "device is not available: {}",
            device.label()
        );
        let abi = device.abi(enabled).with_context(|| {
            format!(
                "device has no usable ABI from [{}]: {}",
                enabled.join(", "),
                device.label()
            )
        })?;
        return Ok((device.serial.clone(), abi.into()));
    }
    let compatible: Vec<_> = devices
        .iter()
        .filter_map(|device| device.abi(enabled).map(|abi| (device, abi)))
        .collect();
    ensure!(
        !compatible.is_empty(),
        "no available Android device supports [{}]; connect and authorize a compatible device or start an emulator.\n{}",
        enabled.join(", "),
        devices
            .iter()
            .map(Device::label)
            .collect::<Vec<_>>()
            .join("\n")
    );
    let index = if compatible.len() == 1 {
        0
    } else {
        ensure!(
            io::stdin().is_terminal(),
            "multiple compatible Android devices; pass --device SERIAL (see `gpuiforge devices`)"
        );
        let labels = compatible
            .iter()
            .map(|(device, _)| device.label())
            .collect::<Vec<_>>();
        crate::execute::choose("Choose an Android device", &labels)?
    };
    let (device, abi) = compatible[index];
    eprintln!("Using {} ({abi})", device.serial);
    Ok((device.serial.clone(), abi.into()))
}
