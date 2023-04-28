use std::borrow::Cow;
use bytemuck::{Pod, Zeroable};

use futures_intrusive;
use web_sys::console;
use wgpu::{
    Backends,
    Buffer, BufferUsages, CommandEncoder, ComputePipeline, Device, InstanceDescriptor,
    Queue, RenderPipeline, ShaderStages, util, util::DeviceExt,
};

pub async fn run_compute_shader(shader: &str, input_data: Vec<f32>) -> Vec<f32> {
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
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader)),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &cs_module,
        entry_point: "main",
    });
    let input: &[u8] = bytemuck::cast_slice(&input_data);
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Buffer"),
        contents: input,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
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
                resource: input_buffer.as_entire_binding(),
            }
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(3, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&input_buffer, 0, &output_buffer, 0, input.len() as u64);
    queue.submit(Some(encoder.finish()));
    let buf_slice = output_buffer.slice(..);
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






pub async fn mix_and_color(shader: &str, input_data: Vec<f32>, color_vec: Vec<f32>) -> Vec<f32> {
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
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader)),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &cs_module,
        entry_point: "main",
    });

    let input: &[u8] = bytemuck::cast_slice(&input_data);

    // let color_vec: Vec<f32> =


    const COLORS_MAX_LENGTH: usize = 30;
    #[derive(Copy, Clone)]
    #[repr(C)]
    struct InputColorsAndLengths {
        intensity_length: u32,
        color_length: u32,
        colors: [f32; COLORS_MAX_LENGTH],
    }
    unsafe impl Zeroable for InputColorsAndLengths {}

    unsafe impl Pod for InputColorsAndLengths {}


    let mut color_arr: [f32; COLORS_MAX_LENGTH] = [0.0; COLORS_MAX_LENGTH];
    color_arr[..color_vec.len()].copy_from_slice(&color_vec);

    let input_lengths = InputColorsAndLengths {
        intensity_length: input_data.len() as u32,
        color_length: color_vec.len() as u32,
        colors: color_arr,
    };
    let input_lengths_slice = &[input_lengths];

    let input_lengths: &[u8] = bytemuck::cast_slice(input_lengths_slice);

    let input_intensity_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Buffer"),
        contents: input,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });
    let input_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Color and Lengths Buffer"),
        contents: input_lengths,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });
    let output_length: usize = 4 * 4 * input_data.len() as usize / (color_vec.len() / 3);
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let map_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
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
                resource: input_intensity_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_color_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    let num_elements = input_data.len();
    let workgroup_size = 256;
    let num_workgroups = (num_elements + workgroup_size - 1) / workgroup_size;

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(num_workgroups as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &map_buffer, 0, output_length as u64);
    queue.submit(Some(encoder.finish()));
    let buf_slice = map_buffer.slice(..);
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

