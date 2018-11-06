use std::cmp::max;

use memory::{Block, Heaps};

use crate::{
    buffer,
    escape::{Escape, Terminal},
    image,
};

/// Resource manager.
/// It can be used to create and destroy resources such as buffers and images.
#[derive(Debug, Derivative)]
#[derivative(Default(bound = ""))]
pub struct Resources<B: gfx_hal::Backend> {
    buffers: Terminal<buffer::Inner<B>>,
    images: Terminal<image::Inner<B>>,

    dropped_buffers: Vec<buffer::Inner<B>>,
    dropped_images: Vec<image::Inner<B>>,
}

impl<B> Resources<B>
where
    B: gfx_hal::Backend,
{
    /// Create new `Resources` instance.
    pub fn new() -> Self {
        Default::default()
    }

    /// Create a buffer and bind to the memory that support intended usage.
    pub fn create_buffer(
        &mut self,
        device: &impl gfx_hal::Device<B>,
        heaps: &mut Heaps<B>,
        align: u64,
        size: u64,
        usage: impl buffer::Usage,
    ) -> Result<buffer::Buffer<B>, failure::Error> {
        let buf = unsafe {
            device.create_buffer(size, usage.flags())
        }?;
        let reqs = unsafe {
            device.get_buffer_requirements(&buf)
        };
        let block = heaps.allocate(
            device,
            reqs.type_mask as u32,
            usage.memory(),
            reqs.size,
            max(reqs.alignment, align),
        )?;

        let buf = unsafe {
            device.bind_buffer_memory(block.memory(), block.range().start, buf)
        }?;

        Ok(buffer::Buffer {
            inner: self.buffers.escape(buffer::Inner {
                raw: buf,
                block,
                relevant: relevant::Relevant,
            }),
            info: buffer::Info {
                align,
                size,
                usage: usage.flags(),
            }
        })
    }

    /// Destroy buffer.
    /// Buffer can be dropped but this method reduces overhead.
    pub fn destroy_buffer(&mut self, buffer: buffer::Buffer<B>) {
        self.dropped_buffers.push(Escape::into_inner(buffer.inner));
    }

    /// Drop inner buffer representation.
    ///
    /// # Safety
    ///
    /// Device must not attempt to use the buffer.
    unsafe fn destroy_buffer_inner(
        inner: buffer::Inner<B>,
        device: &impl gfx_hal::Device<B>,
        heaps: &mut Heaps<B>,
    ) {
        device.destroy_buffer(inner.raw);
        heaps.free(device, inner.block);
        inner.relevant.dispose();
    }

    /// Create an image and bind to the memory that support intended usage.
    pub fn create_image(
        &mut self,
        device: &impl gfx_hal::Device<B>,
        heaps: &mut Heaps<B>,
        align: u64,
        kind: gfx_hal::image::Kind,
        levels: gfx_hal::image::Level,
        format: gfx_hal::format::Format,
        tiling: gfx_hal::image::Tiling,
        view_caps: gfx_hal::image::ViewCapabilities,
        usage: impl image::Usage,
    ) -> Result<image::Image<B>, failure::Error> {
        let img = unsafe {
            device.create_image(
                kind,
                levels,
                format,
                tiling,
                usage.flags(),
                view_caps,
            )
        }?;
        let reqs = unsafe {
            device.get_image_requirements(&img)
        };
        let block = heaps.allocate(
            device,
            reqs.type_mask as u32,
            usage.memory(),
            reqs.size,
            max(reqs.alignment, align),
        )?;

        let img = unsafe {
            device
                .bind_image_memory(block.memory(), block.range().start, img)
        }?;

        Ok(image::Image {
            inner: self.images.escape(image::Inner {
                raw: img,
                block,
                relevant: relevant::Relevant,
            }),
            info: image::Info {
                align,
                kind,
                levels,
                format,
                tiling,
                view_caps,
                usage: usage.flags(),
            },
        })
    }

    /// Destroy image.
    /// Image can be dropped but this method reduces overhead.
    pub fn destroy_image(
        &mut self,
        image: image::Image<B>,
    ) {
        self.dropped_images.push(Escape::into_inner(image.inner));
    }

    /// Drop inner image representation.
    ///
    /// # Safety
    ///
    /// Device must not attempt to use the image.
    unsafe fn destroy_image_inner(
        inner: image::Inner<B>,
        device: &impl gfx_hal::Device<B>,
        heaps: &mut Heaps<B>,
    ) {
        device.destroy_image(inner.raw);
        heaps.free(device, inner.block);
        inner.relevant.dispose();
    }

    /// Recycle dropped resources.
    ///
    /// # Safety
    ///
    /// Device must not attempt to use previously dropped buffers and images.
    pub unsafe fn cleanup(&mut self, device: &impl gfx_hal::Device<B>, heaps: &mut Heaps<B>) {
        for buffer in self.dropped_buffers.drain(..) {
            Self::destroy_buffer_inner(buffer, device, heaps);
        }

        for image in self.dropped_images.drain(..) {
            Self::destroy_image_inner(image, device, heaps);
        }

        self.dropped_buffers.extend(self.buffers.drain());
        self.dropped_images.extend(self.images.drain());
    }
}
