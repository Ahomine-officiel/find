// wgpu initialization shared by windowed and headless modes.

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: String,
}

pub fn create_instance() -> wgpu::Instance {
    wgpu::Instance::new(&wgpu::InstanceDescriptor::default())
}

pub async fn create_device(instance: &wgpu::Instance) -> Result<(wgpu::Adapter, Gpu), String> {
    let req = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    };
    let adapter = instance
        .request_adapter(&req)
        .await
        .ok_or("No suitable GPU adapter found")?;

    let info = adapter.get_info();
    let adapter_info = format!("{} ({:?})", info.name, info.backend);

    let attempts: [&str; 3] = ["default", "downlevel", "webgl2"];
    let mut last_err = String::new();
    for a in attempts {
        let limits = match a {
            "default" => wgpu::Limits::default(),
            "downlevel" => wgpu::Limits::downlevel_defaults(),
            _ => wgpu::Limits::downlevel_webgl2_defaults(),
        };
        let desc = wgpu::DeviceDescriptor {
            label: Some("find-the-needle"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::default(),
        };
        match adapter.request_device(&desc, None).await {
            Ok((device, queue)) => {
                return Ok((
                    adapter,
                    Gpu { device, queue, adapter_info },
                ));
            }
            Err(e) => last_err = format!("{e}"),
        }
    }
    Err(format!("Failed to create device: {last_err}"))
}
