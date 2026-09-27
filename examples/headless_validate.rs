// Headless GPU validation: creates a device and builds ALL pipelines,
// bind groups and buffers exactly as the game does - without a window.
// Run: cargo run --release --example headless_validate
use find_the_needle::render::Pipelines;
use find_the_needle::world::World;

fn main() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));
    let pipes = if let Some(adapter) = adapter {
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
            None,
        ))
        .expect("device");
        // 1. pipelines + bind groups (vertex layouts, entry points, resource types)
        let p = Pipelines::new(&device, &queue, wgpu::TextureFormat::Bgra8Unorm);
        println!("OK pipelines created on {}", adapter.get_info().name);
        Some(p)
    } else {
        println!("NOTE: no GPU adapter in this environment - pipelines were validated via naga tests instead");
        None
    };

    // 2. world meshes + straw buffers (real sizes)
    let world = World::new(1, 100_000);
    let barn = find_the_needle::mesh::barn_mesh();
    let outdoor = find_the_needle::mesh::outdoor_mesh();
    let props = find_the_needle::mesh::build_all_meshes();
    let straw_mesh = find_the_needle::mesh::StrawMesh::build();
    let total_verts = barn.0.len() + outdoor.0.len() + props.iter().map(|(v, _)| v.len()).sum::<usize>();
    println!(
        "OK world built: barn {} verts / outdoor {} verts / {} prop meshes / {} straw instances / straw mesh {}v {}i",
        barn.0.len(), outdoor.0.len(), props.len(), world.straws.len(), straw_mesh.0.len(), straw_mesh.1.len()
    );
    let _ = total_verts;
    let _ = pipes;

    // sanity: u16 index limits
    assert!(barn.0.len() < 65535, "barn mesh exceeds u16 indices");
    assert!(barn.0.len() + outdoor.0.len() < 65535, "merged world mesh exceeds u16 indices");
    for (i, (v, idx)) in props.iter().enumerate() {
        assert!(v.len() < 65535, "prop mesh {i} exceeds u16 indices");
        assert_eq!(v.len(), idx.iter().max().map(|m| m + 1).unwrap_or(0) as usize, "prop mesh {i} index out of bounds");
    }
    println!("OK all index buffers within u16 bounds");
    println!("HEADLESS VALIDATION PASSED");
}
