//! Packs small images into shared texture pages, so quads drawing different
//! sprites, icons and plain fills can share one draw call.

use etagere::{AllocId, AtlasAllocator, size2};
use num_traits::AsPrimitive;

/// Larger images keep their own textures; packing them would waste pages.
pub const MAX_IMAGE: u32 = 512;
const PAGE_SIZE: u32 = 2048;
/// Edge texels are repeated around each image so linear filtering never
/// samples a neighbour, matching clamp-to-edge sampling of a lone texture.
const PADDING: u32 = 1;

struct Page {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    allocator: AtlasAllocator,
    /// Images packed here, not counting the white texel.
    images: usize,
    /// UV of the opaque white texel used by untextured quads.
    white: [f32; 2],
}

pub struct Region {
    pub page: usize,
    /// u0, v0, u1, v1 of the image within its page.
    pub uv: [f32; 4],
    pub size: (u32, u32),
    id: AllocId,
}

pub struct Atlas {
    pages: Vec<Option<Page>>,
    size: u32,
}

impl Atlas {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let mut atlas = Self {
            pages: Vec::new(),
            size: PAGE_SIZE.min(device.limits().max_texture_dimension_2d),
        };
        atlas.add_page(device, queue, layout, sampler);
        atlas
    }

    pub fn fits(&self, width: u32, height: u32) -> bool {
        let limit = MAX_IMAGE.min(self.size - 2 * PADDING);
        (1..=limit).contains(&width) && (1..=limit).contains(&height)
    }

    pub fn bind_group(&self, page: usize) -> &wgpu::BindGroup {
        &self.page(page).bind_group
    }

    /// UV of an opaque white texel on `page` (or the first page).
    pub fn white(&self, page: Option<usize>) -> (usize, [f32; 2]) {
        let page = page.filter(|&p| self.pages.get(p).is_some_and(Option::is_some));
        let page = page.unwrap_or(0);
        (page, self.page(page).white)
    }

    fn page(&self, page: usize) -> &Page {
        self.pages[page].as_ref().expect("atlas page is resident")
    }

    #[cfg(feature = "dev-console")]
    pub fn page_count(&self) -> usize {
        self.pages.iter().flatten().count()
    }

    #[cfg(feature = "dev-console")]
    pub fn page_bytes(&self) -> u64 {
        u64::from(self.size) * u64::from(self.size) * 4
    }

    /// Packs `image`, adding a page when the existing ones are full.
    pub fn insert(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        image: &image::RgbaImage,
    ) -> Option<Region> {
        let (width, height) = image.dimensions();
        if !self.fits(width, height) {
            return None;
        }
        let region = (0..self.pages.len())
            .find_map(|index| self.write(queue, index, image))
            .or_else(|| {
                let index = self.add_page(device, queue, layout, sampler);
                self.write(queue, index, image)
            })?;
        if let Some(page) = &mut self.pages[region.page] {
            page.images += 1;
        }
        Some(region)
    }

    pub fn remove(&mut self, region: &Region) {
        let Some(page) = self.pages.get_mut(region.page).and_then(Option::as_mut) else {
            return;
        };
        page.allocator.deallocate(region.id);
        page.images -= 1;
        // Keep the first page for plain fills; later pages go once they are empty.
        if region.page > 0 && page.images == 0 {
            self.pages[region.page] = None;
        }
    }

    fn add_page(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> usize {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image atlas"),
            size: wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let bind_group = super::Renderer::texture_bind_group(device, layout, sampler, &texture);
        let side = self.size.as_();
        let page = Page {
            texture,
            bind_group,
            allocator: AtlasAllocator::new(size2(side, side)),
            images: 0,
            white: [0.0; 2],
        };
        let index = self
            .pages
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                self.pages.push(None);
                self.pages.len() - 1
            });
        self.pages[index] = Some(page);
        let white = image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]));
        let region = self
            .write(queue, index, &white)
            .expect("an empty atlas page holds a white texel");
        let page = self.pages[index].as_mut().expect("page was just added");
        page.white = [
            f32::midpoint(region.uv[0], region.uv[2]),
            f32::midpoint(region.uv[1], region.uv[3]),
        ];
        index
    }

    fn write(
        &mut self,
        queue: &wgpu::Queue,
        index: usize,
        image: &image::RgbaImage,
    ) -> Option<Region> {
        let size = self.size;
        let page = self.pages[index].as_mut()?;
        let (width, height) = image.dimensions();
        let (padded_w, padded_h) = (width + 2 * PADDING, height + 2 * PADDING);
        let allocation = page
            .allocator
            .allocate(size2(padded_w.as_(), padded_h.as_()))?;
        let min = allocation.rectangle.min;
        let (x, y): (u32, u32) = (min.x.as_(), min.y.as_());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &padded(image),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_w * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: padded_w,
                height: padded_h,
                depth_or_array_layers: 1,
            },
        );
        let scale = 1.0 / AsPrimitive::<f32>::as_(size);
        let (u0, v0): (f32, f32) = ((x + PADDING).as_(), (y + PADDING).as_());
        let (w, h): (f32, f32) = (width.as_(), height.as_());
        Some(Region {
            page: index,
            uv: [u0 * scale, v0 * scale, (u0 + w) * scale, (v0 + h) * scale],
            size: (width, height),
            id: allocation.id,
        })
    }
}

/// The image surrounded by copies of its edge texels.
fn padded(image: &image::RgbaImage) -> Vec<u8> {
    let (width, height) = image.dimensions();
    let pad = PADDING as usize;
    let (w, h) = (width as usize, height as usize);
    let row = (w + 2 * pad) * 4;
    let mut out = Vec::with_capacity(row * (h + 2 * pad));
    let source = image.as_raw();
    for y in 0..h + 2 * pad {
        let line = &source[y.saturating_sub(pad).min(h - 1) * w * 4..][..w * 4];
        for _ in 0..pad {
            out.extend_from_slice(&line[..4]);
        }
        out.extend_from_slice(line);
        for _ in 0..pad {
            out.extend_from_slice(&line[(w - 1) * 4..]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::padded;

    #[test]
    fn padding_repeats_edge_texels() {
        let image =
            image::RgbaImage::from_fn(2, 1, |x, _| image::Rgba([u8::try_from(x + 1).unwrap(); 4]));
        let px = |v: u8| [v; 4];
        let row = [px(1), px(1), px(2), px(2)].concat();
        assert_eq!(padded(&image), [row.clone(), row.clone(), row].concat());
    }
}
