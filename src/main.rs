use std::io::{Cursor, Result};
use pixels::{Pixels, SurfaceTexture};
use pixels::wgpu::Color;
use winit::dpi::LogicalSize;
use winit::event::{WindowEvent, Event};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use image::io::Reader as ImageReader;

const SCREEN_WIDTH: u32		= 640;
const SCREEN_HEIGHT: u32	= 480;
const SKY_BLUE: Color		= Color {r: 0.43, g: 0.82, b: 1.0, a: 1.0};

const R: usize	= 0;
const G: usize	= 1;
const B: usize	= 2;
const A: usize	= 3;

const X: usize	= 0;
const Y: usize	= 1;
const Z: usize	= 2;

fn main()
{
	// Used for wgpu error logging.
	env_logger::init();
	
	// Initialize winit library.
	let event_loop	= EventLoop::new().unwrap();
	let mut input	= WinitInputHelper::new();
	event_loop.set_control_flow(ControlFlow::Poll);
	
	// Create window.
	let window_size = LogicalSize::new
	(
		SCREEN_WIDTH as f64,
		SCREEN_HEIGHT as f64
	);
	let window = WindowBuilder::new()
		.with_title("Voxel Space")
		.with_inner_size(window_size)
		.with_min_inner_size(window_size)
		.build(&event_loop)
		.unwrap();
	
	// Initialize framebuffer.
	let framebuffer_size	= window.inner_size();
	let surface_texture		= SurfaceTexture::new
	(
		framebuffer_size.width,
		framebuffer_size.height,
		&window
	);
	let mut pixels = Pixels::new
	(
		SCREEN_WIDTH,
		SCREEN_HEIGHT,
		surface_texture
	).unwrap();
	pixels.clear_color(SKY_BLUE);
	
	// Construct the camera and the world.
	let mut camera	= Camera::new();
	let terrain_map = TerrainMap::new
	(
		"./maps/color_map.png",
		"./maps/height_map.png"
	);
	
	let _ = event_loop.run(move |event, target_window|
	{
		// Draw the current frame.
		if let Event::WindowEvent {event, ..} = event.clone()
		{
			match event
			{
				WindowEvent::CloseRequested		=> target_window.exit(),
				WindowEvent::RedrawRequested	=>
				{
					camera.draw(pixels.frame_mut(), &terrain_map);
					pixels.render();
				},
				_ => ()
			}
		}
		
		// Handle user input.
		if input.update(&event)
		{
			// Handle camera movement and rotation.
			if input.key_held(KeyCode::KeyW)
				{camera.velocity[2] += camera.acceleration;}
			if input.key_held(KeyCode::KeyA)
				{camera.velocity[0] -= camera.acceleration;}
			if input.key_held(KeyCode::KeyS)
				{camera.velocity[2] -= camera.acceleration;}
			if input.key_held(KeyCode::KeyD)
				{camera.velocity[0] += camera.acceleration;}
			if input.key_held(KeyCode::KeyQ)
				{camera.velocity[1] -= camera.acceleration;}
			if input.key_held(KeyCode::KeyE)
				{camera.velocity[1] += camera.acceleration;}
			
			// Handle window closing.
			if input.key_pressed(KeyCode::Escape)
				|| input.close_requested()
			{
				return;
			}
			
			// Handle window resizing.
			if let Some(size) = input.window_resized()
			{
				let _ = pixels.resize_surface(size.width, size.height);
			}
			
			// Update camera and request a redraw.
			camera.update();
			window.request_redraw();
		}
	});
}

struct Camera
{
	velocity:		[f32; 3],	// x, y, z
	position:		[f32; 3],	// x, y, z
	rotation:		[f32; 2],	// roll, yaw
	far_clip:		f32,
	acceleration:	f32,
	max_speed:		f32,
}

impl Camera
{
	fn new() -> Self
	{
		Self
		{
			velocity:		[0.0, 0.0, 0.0],
			position:		[512.0, 512.0, 0.0],
			rotation:		[0.0, 0.0],
			far_clip:		400.0,
			acceleration:	0.25,
			max_speed:		5.0,
		}
	}
	
	fn update(&mut self)
	{
		// Limit velocity and add it to position.
		for i in 0..3
		{
			if self.velocity[i] > self.max_speed
			{
				self.velocity[i] = self.max_speed;
			}
			self.velocity[i] *= 0.9;
			self.position[i] += self.velocity[i];
		}
	}
	
	fn draw(&self, frame: &mut [u8], terrain_map: &TerrainMap)
	{
		// Must be 4x the length of frame to fit all pixel data.
		let mut new_frame: Vec<u8> = vec![0x00; frame.len() * 3];
		
		// Left far camera frustrum bound.
		let left_far: [f32; 2] = 
		[
			0.0 - self.far_clip,
			self.far_clip,
		];
		// Right far camera frustrum bound.
		let right_far: [f32; 2] = 
		[
			self.far_clip,
			self.far_clip,
		];
		
		// Loop through the screen to cast rays on the terrain.
		for screen_x in 0..SCREEN_WIDTH
		{
			let delta_pos: [f32; 2] = 
			[
				(left_far[X] + (right_far[X] - left_far[X]) / SCREEN_WIDTH as f32 * screen_x as f32) / self.far_clip,
				(left_far[Y] + (right_far[Y] - left_far[Y]) / SCREEN_WIDTH as f32 * screen_x as f32) / self.far_clip,
			];
			let mut ray_pos: [f32; 2] =
			[
				self.position[X],
				self.position[Y],
			];
			
			// Used to cull non-visible terrain.
			let mut max_projected_height: u32 = SCREEN_HEIGHT;
			
			// Cast rays for each pixel from our camera origin to our z-clip.
			for ray_z in 1..(self.far_clip as usize)
			{
				ray_pos[X] += delta_pos[X];
				ray_pos[Y] -= delta_pos[Y];
				
				let map_offset = terrain_map.map_dimensions[X] * 
					ray_pos[Y] as usize + ray_pos[X] as usize;
				let height_on_screen = terrain_map.height_map[map_offset] as u32;
				
				/* Make sure we are only rendering taller pixels than 
					previously drawn for this ray. */
				if height_on_screen < max_projected_height
				{
					for screen_y in 0..height_on_screen
					{
						let pixel_index = SCREEN_WIDTH as usize * screen_y as usize;
						new_frame[pixel_index + R]	= terrain_map.color_map[map_offset + R];
						new_frame[pixel_index + G]	= terrain_map.color_map[map_offset + G];
						new_frame[pixel_index + B]	= terrain_map.color_map[map_offset + B];
					}
					max_projected_height = height_on_screen;
				}
				
				/*let pixel_index = SCREEN_WIDTH as usize * (ray_pos[Y] / 4.0) as usize + (ray_pos[X] / 4.0) as usize;
				new_frame[pixel_index + R]	= 0xFF;
				new_frame[pixel_index + G]	= 0xFF;
				new_frame[pixel_index + B]	= 0xFF;*/
			}
		}
		
		// Copy our new frame into our framebuffer in chunks of 4 bytes. (RGBA).
		for(i, pixel) in frame.chunks_exact_mut(4).enumerate()
		{
			pixel[R] = new_frame[i];
			pixel[G] = new_frame[i];
			pixel[B] = new_frame[i];
			
			// TODO: Remove the alpha component.
			pixel[A] = 0xFF;
		}
	}
}

struct TerrainMap
{
	map_dimensions:	[usize; 2],
	color_map:		Vec<u8>,
	height_map:		Vec<u8>,
}

impl TerrainMap
{
	/// Opens and reads a color and height map.
	fn new(color_map_path: &str, height_map_path: &str) -> Self
	{
		let mut new_map = TerrainMap
		{
			map_dimensions:	[0, 0],
			color_map:		Vec::new(),
			height_map:		Vec::new(),
		};
		
		// Read the image files.
		let color_map_img	= ImageReader::open(color_map_path)
			.unwrap()
			.decode()
			.unwrap();
		let height_map_img	= ImageReader::open(height_map_path)
			.unwrap()
			.decode()
			.unwrap();
		
		new_map.map_dimensions	= 
		[
			color_map_img.width() as usize,
			color_map_img.height() as usize
		];
		
		new_map.color_map		= color_map_img.to_rgba8().into_vec();
		new_map.height_map		= height_map_img.to_rgba8().into_vec();
		
		new_map
	}
}