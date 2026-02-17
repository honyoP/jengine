use wgpu::util::DeviceExt;

pub struct Atlas {
    pub texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub cols: u32,
    pub rows: u32,
    pub tile_w: u32,
    pub tile_h: u32,
}

impl Atlas {
    pub fn from_png(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        png_bytes: &[u8],
        tile_w: u32,
        tile_h: u32,
    ) -> Self {
        let img = image::load_from_memory(png_bytes)
            .expect("failed to load tileset PNG")
            .to_rgba8();
        let (img_w, img_h) = img.dimensions();
        let cols = img_w / tile_w;
        let rows = img_h / tile_h;

        let size = wgpu::Extent3d {
            width: img_w,
            height: img_h,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("atlas"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &img,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self { texture_view, sampler, cols, rows, tile_w, tile_h }
    }

    /// Returns (uv_min, uv_max) for a given tile index (row-major order).
    pub fn uv_for_index(&self, index: u32) -> ([f32; 2], [f32; 2]) {
        let col = index % self.cols;
        let row = index / self.cols;
        let total_w = (self.cols * self.tile_w) as f32;
        let total_h = (self.rows * self.tile_h) as f32;

        let u_min = (col * self.tile_w) as f32 / total_w;
        let v_min = (row * self.tile_h) as f32 / total_h;
        let u_max = ((col + 1) * self.tile_w) as f32 / total_w;
        let v_max = ((row + 1) * self.tile_h) as f32 / total_h;

        ([u_min, v_min], [u_max, v_max])
    }
}
