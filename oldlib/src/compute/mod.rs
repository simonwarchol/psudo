use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use argmin::core::{CostFunction, Error, Executor, Operator, OptimizationResult, State};
use argmin::solver::simulatedannealing::{Anneal, SATempFunc};
use argmin::solver::simulatedannealing::SimulatedAnnealing;
use bytemuck::{Pod, Zeroable};
use futures::executor::block_on;
use futures_intrusive;
use rand::distributions::Uniform;
use rand::prelude::*;
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rayon::prelude::*;
use simulated_annealing::{APF, Bounds, NeighbourMethod, Point, SA, Schedule, Status};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;
use wgpu::{
    Backends, BindGroup,
    Buffer, BufferUsages, CommandEncoder, ComputePipeline, Device, InstanceDescriptor,
    Queue, RenderPipeline, ShaderStages, util, util::DeviceExt,
};

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
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let input_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Color and Lengths Buffer"),
        contents: input_lengths,
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let output_length: usize = 4 * 4 * input_data.len() as usize / (color_vec.len() / 3);
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let map_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
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

async fn create_device_queue_pipeline(shader: &str) -> (Device, Queue, ComputePipeline) {
    console::log_1(&"66".into());

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        // Optional fields:
        backends: (wgpu::Backends::METAL | wgpu::Backends::BROWSER_WEBGPU),
        dx12_shader_compiler: Default::default(),
    });
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("Failed to find an appropriate adapter");
    console::log_1(&"7".into());


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
    return (device, queue, pipeline);
}


async fn dispatch_compute_shader(num_elements: usize, device: Device, pipeline: &ComputePipeline,
                                 queue: Queue, bind_group: &BindGroup, output_buffer: Buffer,
                                 map_buffer: Buffer, output_length: usize) -> Vec<f32> {
    let workgroup_size = 256;
    let num_workgroups = (num_elements + workgroup_size - 1) / workgroup_size;

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(pipeline);
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

pub async fn saturated_pixel_penalty(shader: &str, input_data: Vec<f32>, color_vec: Vec<f32>) -> Vec<f32> {
    let (device, queue, pipeline) = create_device_queue_pipeline(shader).await;
    let input: &[u8] = bytemuck::cast_slice(&input_data);
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
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let input_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Color and Lengths Buffer"),
        contents: input_lengths,
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let output_length: usize = 4 * input_data.len() / (color_vec.len() / 3) as usize;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        mapped_at_creation: false,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
    });
    let map_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
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
    dispatch_compute_shader(input_data.len(), device, &pipeline, queue,
                            &bind_group, output_buffer,
                            map_buffer, output_length).await
}


pub async fn diff_penalty(shader: &str, input_data: &Vec<f32>, color_vec: Vec<f32>) -> Vec<f32> {
    let (device, queue, pipeline) = create_device_queue_pipeline(shader).await;
    let input: &[u8] = bytemuck::cast_slice(&input_data);
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
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let input_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Color and Lengths Buffer"),
        contents: input_lengths,
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let output_length: usize = 4 * input_data.len() / (color_vec.len() / 3) as usize;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        mapped_at_creation: false,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let map_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_length as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
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

    dispatch_compute_shader(input_data.len(), device, &pipeline, queue,
                            &bind_group, output_buffer,
                            map_buffer, output_length).await
}


struct Loss {
    rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    intensities: Vec<f32>,
}

impl Loss {
    pub fn new(intensities: Vec<f32>) -> Self {
        Self {
            rng: Arc::new(Mutex::new(Xoshiro256PlusPlus::from_entropy())),
            intensities,
        }
    }
}

impl CostFunction for Loss {
    type Param = Vec<f32>;
    type Output = f32;


    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        let init_param = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let intensities = vec![1.0, 2.0, 3.0];
        let future_result = async {
            vec![1.0]
        };
        // Console log
        let result: Vec<f32> = futures::executor::block_on(future_result);

        // let future_result = diff_penalty(include_str!("diff.wgsl"), &intensities, init_param.clone());

        // let future_result = diff_penalty(include_str!("diff.wgsl"), &self.intensities, param.clone());
        // let result: Vec<f32> = futures::executor::block_on(future_result);
        // let result: Vec<f32> = future_result.block_on();
        // let result_sum: f32 = result.par_iter().sum();
        // let mean: f32 = result_sum / result.len() as f32;
        Ok(0.1)
    }
}

impl Anneal for Loss {
    type Param = Vec<f32>;
    type Output = Vec<f32>;
    type Float = f32;

    /// Anneal a parameter vector
    fn anneal(&self, param: &Vec<f32>, temp: f32) -> Result<Vec<f32>, Error> {
        let mut param_n = param.clone();
        let mut rng = self.rng.lock().unwrap();
        let distr = Uniform::from(0..param.len());
        // Perform modifications to a degree proportional to the current temperature `temp`.
        for _ in 0..(temp.floor() as u64 + 1) {
            // Compute random index of the parameter vector using the supplied random number
            // generator.
            let idx = rng.sample(distr);
            // Compute random number in [0.1, 0.1].
            let val = rng.sample(Uniform::new_inclusive(-0.1, 0.1));
            // modify previous parameter value at random position `idx` by `val`
            param_n[idx] += val;

            // check if bounds are violated. If yes, project onto bound.
            param_n[idx] = param_n[idx].clamp(0.0, 1.0);
        }
        Ok(param_n)
    }
}


pub fn optimize(intensities: Vec<f32>) -> Result<(), Error> {
    let clone_intensity = intensities.clone();
    let cost_function = Loss::new(intensities);

    let initial_temp = 100.0;
    let max_iter = 1000;

    //
    // Console log
    spawn_local(async move {
        let init_param = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let intensities = vec![1.0, 2.0, 3.0];
        let future_result = diff_penalty(include_str!("diff.wgsl"), &intensities, init_param.clone());
        let result = future_result.await;
        // console log "Hello, world!"
        console::log_1(&"Spawn".into());
    });
    console::log_1(&"Prawn".into());

    // let solver = SimulatedAnnealing::new(initial_temp)?;
    // // Optional: Define temperature function (defaults to `SATempFunc::TemperatureFast`)
    // let res = Executor::new(cost_function, solver)
    //     .configure(|state| {
    //         state
    //             .param(init_param)
    //             .max_iters(max_iter)
    //     })
    //     // Optional: Attach an observer
    //     .run()?;
    // // Print result
    // println!("{res}");
    // let best_param = res.state().get_best_param().unwrap();
    // println!("Best parameter: {:?}", *best_param);
    Ok(())
}

