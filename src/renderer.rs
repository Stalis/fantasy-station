use sdl3::gpu::{
    ColorTargetDescription, ColorTargetInfo, Device, Filter, GraphicsPipeline,
    GraphicsPipelineTargetInfo, LoadOp, Sampler, SamplerAddressMode, SamplerCreateInfo,
    ShaderFormat, ShaderStage, StoreOp, Texture, TextureCreateInfo, TextureFormat, TextureRegion,
    TextureSamplerBinding, TextureTransferInfo, TextureType, TextureUsage, TransferBuffer,
    TransferBufferUsage, Viewport,
};
use sdl3::pixels::Color;
use sdl3::video::Window;

const WIDTH: u32 = super::SCREEN_WIDTH as u32;
const HEIGHT: u32 = super::SCREEN_HEIGHT as u32;
const BYTE_COUNT: u32 = WIDTH * HEIGHT * 4;

#[cfg(target_os = "macos")]
const FORMAT: ShaderFormat = ShaderFormat::MSL;
#[cfg(not(target_os = "macos"))]
const FORMAT: ShaderFormat = ShaderFormat::SPIRV;
#[cfg(target_os = "macos")]
const ENTRY: &std::ffi::CStr = c"shader_main";
#[cfg(not(target_os = "macos"))]
const ENTRY: &std::ffi::CStr = c"main";

#[cfg(target_os = "macos")]
const VERTEX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fullscreen.vert.msl"));
#[cfg(not(target_os = "macos"))]
const VERTEX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fullscreen.vert.spv"));
#[cfg(target_os = "macos")]
const SOFT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/classic_soft.frag.msl"));
#[cfg(not(target_os = "macos"))]
const SOFT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/classic_soft.frag.spv"));
#[cfg(target_os = "macos")]
const PLAIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/passthrough.frag.msl"));
#[cfg(not(target_os = "macos"))]
const PLAIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/passthrough.frag.spv"));

// The claimed window must be released before either the device or window is destroyed.
struct ClaimedWindow {
    device: Device,
    window: Window,
}

impl Drop for ClaimedWindow {
    fn drop(&mut self) {
        unsafe {
            sdl3::sys::gpu::SDL_ReleaseWindowFromGPUDevice(self.device.raw(), self.window.raw());
        }
    }
}

pub struct Renderer {
    soft_pipeline: GraphicsPipeline,
    plain_pipeline: GraphicsPipeline,
    texture: Texture<'static>,
    sampler: Sampler,
    transfer: TransferBuffer,
    claimed: ClaimedWindow,
}

impl Renderer {
    pub fn new(window: Window) -> Result<Self, String> {
        let device = Device::new(FORMAT, false)
            .and_then(|device| device.with_window(&window))
            .map_err(|error| error.to_string())?;
        let claimed = ClaimedWindow { device, window };
        let device = &claimed.device;

        let vertex = device
            .create_shader()
            .with_code(FORMAT, VERTEX, ShaderStage::Vertex)
            .with_entrypoint(ENTRY)
            .build()
            .map_err(|error| error.to_string())?;
        let pipeline = |code| -> Result<GraphicsPipeline, String> {
            let fragment = device
                .create_shader()
                .with_code(FORMAT, code, ShaderStage::Fragment)
                .with_entrypoint(ENTRY)
                .with_samplers(1)
                .build()
                .map_err(|error| error.to_string())?;
            let targets = [ColorTargetDescription::new()
                .with_format(device.get_swapchain_texture_format(&claimed.window))];
            device
                .create_graphics_pipeline()
                .with_vertex_shader(&vertex)
                .with_fragment_shader(&fragment)
                .with_target_info(
                    GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&targets),
                )
                .build()
                .map_err(|error| error.to_string())
        };
        let soft_pipeline = pipeline(SOFT)?;
        let plain_pipeline = pipeline(PLAIN)?;
        let texture = device
            .create_texture(
                TextureCreateInfo::new()
                    .with_type(TextureType::_2D)
                    .with_format(TextureFormat::R8g8b8a8Unorm)
                    .with_usage(TextureUsage::SAMPLER)
                    .with_width(WIDTH)
                    .with_height(HEIGHT)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1),
            )
            .map_err(|error| error.to_string())?;
        let sampler = device
            .create_sampler(
                SamplerCreateInfo::new()
                    .with_min_filter(Filter::Nearest)
                    .with_mag_filter(Filter::Nearest)
                    .with_address_mode_u(SamplerAddressMode::ClampToEdge)
                    .with_address_mode_v(SamplerAddressMode::ClampToEdge)
                    .with_address_mode_w(SamplerAddressMode::ClampToEdge),
            )
            .map_err(|error| error.to_string())?;
        let transfer = device
            .create_transfer_buffer()
            .with_usage(TransferBufferUsage::UPLOAD)
            .with_size(BYTE_COUNT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            soft_pipeline,
            plain_pipeline,
            texture,
            sampler,
            transfer,
            claimed,
        })
    }

    pub fn window_mut(&mut self) -> &mut Window {
        &mut self.claimed.window
    }

    pub fn render(&mut self, framebuffer: &[Color], classic_soft: bool) -> Result<(), String> {
        if framebuffer.len() != (WIDTH * HEIGHT) as usize {
            return Err(format!(
                "framebuffer must contain {} pixels",
                WIDTH * HEIGHT
            ));
        }
        let device = &self.claimed.device;
        // The safe SDL wrapper does not check whether mapping returned null.
        let mapped = unsafe {
            sdl3::sys::gpu::SDL_MapGPUTransferBuffer(device.raw(), self.transfer.raw(), true)
        };
        if mapped.is_null() {
            return Err(sdl3::get_error().to_string());
        }
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(mapped.cast::<u8>(), BYTE_COUNT as usize) };
        for (dst, color) in bytes.chunks_exact_mut(4).zip(framebuffer) {
            dst.copy_from_slice(&[color.r, color.g, color.b, color.a]);
        }
        unsafe { sdl3::sys::gpu::SDL_UnmapGPUTransferBuffer(device.raw(), self.transfer.raw()) };

        let mut commands = device
            .acquire_command_buffer()
            .map_err(|error| error.to_string())?;
        let copy = match device.begin_copy_pass(&commands) {
            Ok(pass) => pass,
            Err(error) => {
                commands.cancel();
                return Err(error.to_string());
            }
        };
        copy.upload_to_gpu_texture(
            TextureTransferInfo::new()
                .with_transfer_buffer(&self.transfer)
                .with_pixels_per_row(WIDTH)
                .with_rows_per_layer(HEIGHT),
            TextureRegion::new()
                .with_texture(&self.texture)
                .with_width(WIDTH)
                .with_height(HEIGHT)
                .with_depth(1),
            true,
        );
        device.end_copy_pass(copy);

        let swapchain = match commands.wait_and_acquire_swapchain_texture(&self.claimed.window) {
            Ok(Some(texture)) => texture,
            Ok(None) => {
                let success = unsafe { sdl3::sys::gpu::SDL_SubmitGPUCommandBuffer(commands.raw()) };
                return if success {
                    Ok(())
                } else {
                    Err(sdl3::get_error().to_string())
                };
            }
            Err(error) => {
                let message = error.to_string();
                unsafe { sdl3::sys::gpu::SDL_CancelGPUCommandBuffer(commands.raw()) };
                return Err(message);
            }
        };
        let (x, y, width, height) = letterbox(swapchain.width(), swapchain.height());
        let targets = [ColorTargetInfo::default()
            .with_texture(&swapchain)
            .with_load_op(LoadOp::CLEAR)
            .with_store_op(StoreOp::STORE)
            .with_clear_color(Color::BLACK)];
        let pass = match device.begin_render_pass(&commands, &targets, None) {
            Ok(pass) => pass,
            Err(error) => {
                let message = error.to_string();
                unsafe { sdl3::sys::gpu::SDL_SubmitGPUCommandBuffer(commands.raw()) };
                return Err(message);
            }
        };
        device.set_viewport(&pass, Viewport::new(x, y, width, height, 0.0, 1.0));
        pass.bind_graphics_pipeline(if classic_soft {
            &self.soft_pipeline
        } else {
            &self.plain_pipeline
        });
        pass.bind_fragment_samplers(
            0,
            &[TextureSamplerBinding::new()
                .with_texture(&self.texture)
                .with_sampler(&self.sampler)],
        );
        pass.draw_primitives(3, 1, 0, 0);
        device.end_render_pass(pass);
        drop(targets);
        drop(swapchain);
        commands.submit().map_err(|error| error.to_string())
    }
}

fn letterbox(width: u32, height: u32) -> (f32, f32, f32, f32) {
    let scale = (width as f32 / WIDTH as f32).min(height as f32 / HEIGHT as f32);
    let w = WIDTH as f32 * scale;
    let h = HEIGHT as f32 * scale;
    ((width as f32 - w) / 2.0, (height as f32 - h) / 2.0, w, h)
}

#[cfg(test)]
mod tests {
    use super::letterbox;

    #[test]
    fn letterboxing_tracks_swapchain_size() {
        assert_eq!(letterbox(960, 720), (0.0, 0.0, 960.0, 720.0));
        assert_eq!(letterbox(1280, 720), (160.0, 0.0, 960.0, 720.0));
        assert_eq!(letterbox(640, 600), (0.0, 60.0, 640.0, 480.0));
    }
}
