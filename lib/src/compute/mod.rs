use std::borrow::Cow;

use futures_intrusive;
use web_sys::console;
use wgpu::{
    Backends,
    Buffer, BufferUsages, CommandEncoder, ComputePipeline, Device, InstanceDescriptor,
    Queue, RenderPipeline, ShaderStages, util, util::DeviceExt,
};

pub async fn wgpu_stuff_v2(shader: &str) -> Vec<f32> {
    const NUM_COLORS: usize = 3;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        // Optional fields:
        backends: (wgpu::Backends::METAL | wgpu::Backends::BROWSER_WEBGPU),
        dx12_shader_compiler: Default::default(),
    });
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("Failed to find an appropriate adapter");
    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
            label: None,
        },
        None,
    ).await.expect("Failed to create device");
    let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &cs_module,
        entry_point: "main",
    });
    let srgb_floats: &[f32; 9] = &[0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    let input: &[u8] = bytemuck::bytes_of(srgb_floats);
    let srgb_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sRGB Buffer"),
        contents: input,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });
    let xyz_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: input.len() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: srgb_buffer.as_entire_binding(),
            }
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(9, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&srgb_buffer, 0, &xyz_buffer, 0, input.len() as u64);
    queue.submit(Some(encoder.finish()));
    let buf_slice = xyz_buffer.slice(..);
    let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
    buf_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    device.poll(wgpu::Maintain::Wait);
    if let Some(Ok(())) = receiver.receive().await {
        let data_raw = &*buf_slice.get_mapped_range();
        let data = bytemuck::cast_slice(data_raw);
        return data.to_vec();
    } else {
        panic!("failed to receive data from GPU");
    }
}
