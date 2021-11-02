use std::{iter, fs};
use std::path::PathBuf;
use std::ops::{DerefMut, Deref};
use std::time::Instant;
use wgpu_mc::mc::resource::{ResourceProvider, ResourceType};
use wgpu_mc::mc::datapack::NamespacedId;
use wgpu_mc::mc::block::{BlockDirection, BlockState, BlockModel};
use wgpu_mc::mc::chunk::{ChunkSection, Chunk, CHUNK_AREA, CHUNK_HEIGHT, CHUNK_SECTION_HEIGHT, CHUNK_SECTIONS_PER};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::event::{Event, WindowEvent, KeyboardInput, VirtualKeyCode, ElementState};
use wgpu_mc::{Renderer, ShaderProvider, HasWindowSize, WindowSize};
use futures::executor::block_on;
use winit::window::Window;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use cgmath::InnerSpace;

struct SimpleResourceProvider {
    pub asset_root: PathBuf
}

struct SimpleShaderProvider {
    pub shader_root: PathBuf
}

impl ResourceProvider for SimpleResourceProvider {

    fn get_bytes(&self, t: ResourceType, id: &NamespacedId) -> Vec<u8> {

        let paths: Vec<&str> = match id {
            NamespacedId::Resource(res) => {
                res.1.split("/").take(2).collect()
            },
            _ => unreachable!()
        };

        let path = *paths.first().unwrap();
        let resource = format!("{}.png", *paths.last().unwrap());

        match t {
            ResourceType::Texture => {
                let real_path = self.asset_root.join("minecraft").join("textures").join(path).join(resource);
                fs::read(real_path).unwrap()
            }
        }

    }

}

impl ShaderProvider for SimpleShaderProvider {
    fn get_shader(&self, name: &str) -> String {
        let path = self.shader_root.join(name);
        String::from_utf8(fs::read(path).expect(&format!("Shader {} does not exist", name))).unwrap()
    }
}

struct WinitWindowWrapper {
    window: Window
}

impl HasWindowSize for WinitWindowWrapper {
    fn get_window_size(&self) -> WindowSize {
        WindowSize {
            width: self.window.inner_size().width,
            height: self.window.inner_size().height,
        }
    }
}

unsafe impl HasRawWindowHandle for WinitWindowWrapper {

    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }

}

fn main() {
    let event_loop = EventLoop::new();
    let title = "wgpu-mc test";
    let window = winit::window::WindowBuilder::new()
        .with_title(title)
        .build(&event_loop)
        .unwrap();

    let wrapper = WinitWindowWrapper {
        window
    };

    let sp = SimpleShaderProvider {
        shader_root: crate_root::root().unwrap().join("res").join("shaders"),
    };

    let rsp = SimpleResourceProvider {
        asset_root: crate_root::root().unwrap().join("res").join("assets"),
    };

    let mc_root = crate_root::root()
        .unwrap()
        .join("res")
        .join("assets")
        .join("minecraft");

    let mut state = block_on(Renderer::new(&wrapper, Box::new(sp)));

    println!("Loading block models");
    state.mc.load_block_models(mc_root);
    println!("Generating texture atlas");
    state.mc.generate_block_texture_atlas(&rsp, &state.device, &state.queue, &state.pipelines.layouts.texture_bind_group_layout);
    println!("Generating blocks");
    state.mc.generate_blocks(&state.device, &rsp);

    let window = wrapper.window;

    println!("Starting rendering");
    begin_rendering(event_loop, window, state);
}

fn begin_rendering(mut event_loop: EventLoop<()>, mut window: Window, mut state: Renderer) {
    use futures::executor::block_on;

    let mut block_layers = Vec::new();

    (0..30).for_each(|_| block_layers.push(*state.mc.block_indices.get("minecraft:block/stone").unwrap()));
    (0..19).for_each(|_| block_layers.push(*state.mc.block_indices.get("minecraft:block/dirt").unwrap()));

    block_layers.push(*state.mc.block_indices.get("minecraft:block/grass_block").unwrap());



    // let chunks = (0..8).map(|x| {
    //     (0..8).map(|z| {
    //         let mut sections = Box::new([ChunkSection { empty: true, blocks: [BlockState {
    //             block: None,
    //             // block: None,
    //             direction: BlockDirection::North,
    //             damage: 0,
    //             transparency: false
    //         }; 256] }; 256]);
    //
    //         (0..50).for_each(|y| {
    //             sections.deref_mut()[y] = ChunkSection {
    //                 empty: false,
    //                 blocks: [BlockState {
    //                     block: Option::Some(*block_layers.get(y).unwrap()),
    //                     direction: BlockDirection::North,
    //                     damage: 0,
    //                     transparency: true
    //                 }; 256]
    //             }
    //         });
    //
    //         let mut chunk = Chunk {
    //             pos: (x, z),
    //             sections,
    //             vertices: None,
    //             vertex_buffer: None,
    //             vertex_count: 0
    //         };
    //
    //         chunk.generate_vertices(&state.mc.blocks, x*16, z*16);
    //         chunk.upload_buffer(&state.device);
    //
    //         chunk
    //     }).collect::<Vec<Chunk>>()
    // }).flatten().collect::<Vec<Chunk>>();

    let mut blocks = [
        BlockState {
            block: None,
            direction: BlockDirection::North,
            damage: 0,
            transparency: false
        }; 256
    ];
    
    blocks[0] = BlockState {
        block: state.mc.block_indices.get("minecraft:block/stone").cloned(),
        direction: BlockDirection::North,
        damage: 0,
        transparency: false
    };
    
    let section = ChunkSection {
        empty: false,
        blocks
    };

    let mut sections = [ChunkSection {
        empty: true,
        blocks: [BlockState {
            block: None,
            direction: BlockDirection::North,
            damage: 0,
            transparency: false
        }; CHUNK_AREA]
    }; CHUNK_SECTIONS_PER];
    sections[0] = section;

    let mut chunk = Chunk {
        pos: (0, 0),

        sections: Box::new(sections),
        vertices: None,
        vertex_buffer: None,
        vertex_count: 0
    };

    let instant = Instant::now();

    chunk.generate_vertices(&state.mc.blocks, (0, 0));
    println!("Time to generate chunk mesh: {}", Instant::now().duration_since(instant).as_millis());

    chunk.upload_buffer(&state.device);

    let chunks = [chunk];

    let mut frame_begin = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared => window.request_redraw(),
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        WindowEvent::KeyboardInput { input, .. } => match input {
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Space),
                                ..
                            } => {
                                //Update a block and re-generate the chunk mesh for testing

                                //removed atm for testing
                            },

                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Down),
                                ..
                            } => {
                                state.mc.camera.pitch += 0.1;
                            },
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Up),
                                ..
                            } => {
                                state.mc.camera.pitch -= 0.1;
                            },
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Left),
                                ..
                            } => {
                                state.mc.camera.yaw -= 0.1;
                            },
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Right),
                                ..
                            } => {
                                state.mc.camera.yaw += 0.1;
                            },
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Q),
                                ..
                            } => {
                                state.mc.camera.position.y -= 0.1;
                            },
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::E),
                                ..
                            } => {
                                state.mc.camera.position.y += 0.1;
                            },

                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::W),
                                ..
                            } => {
                                let direction: cgmath::Vector3<f32> = (state.mc.camera.yaw.cos(), state.mc.camera.pitch.sin(), state.mc.camera.yaw.sin()).into();
                                state.mc.camera.position += direction.normalize();
                            },

                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            } => {
                                *control_flow = ControlFlow::Exit;
                            }
                            _ => {}
                        },
                        WindowEvent::Resized(physical_size) => {
                            &state.resize(WindowSize {
                                width: physical_size.width,
                                height: physical_size.height
                            });
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            &state.resize(WindowSize {
                                width: new_inner_size.width,
                                height: new_inner_size.height
                            });
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(_) => {
                &state.update();
                &state.render(&chunks);

                let delta = Instant::now().duration_since(frame_begin).as_millis()+1; //+1 so we don't divide by zero
                frame_begin = Instant::now();

                // println!("Frametime {}, FPS {}", delta, 1000/delta);
            }
            _ => {}
        }
    });
}