//! Safe wrapper around the vendored libvpx VP9 decoder.

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::marker::PhantomData;
use std::ptr;

use anyhow::{Result, bail, ensure};

#[allow(non_camel_case_types)]
mod ffi {
    use std::ffi::{c_char, c_int, c_long, c_uint, c_void};

    pub type vpx_codec_err_t = c_int;
    pub const VPX_CODEC_OK: vpx_codec_err_t = 0;
    // VPX_IMAGE_ABI_VERSION (5) + 4 for the codec ABI + 3 for the decoder ABI.
    pub const VPX_DECODER_ABI_VERSION: c_int = 12;
    pub const VPX_IMG_FMT_HIGHBITDEPTH: c_int = 0x800;
    // vp8dx.h: VP8_DECODER_CTRL_ID_START (256) + position in vp8_dec_control_id.
    pub const VP9D_SET_ROW_MT: c_int = 268;

    #[repr(C)]
    pub struct vpx_codec_iface_t {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct vpx_codec_dec_cfg_t {
        pub threads: c_uint,
        pub w: c_uint,
        pub h: c_uint,
    }

    #[repr(C)]
    pub struct vpx_codec_ctx_t {
        pub name: *const c_char,
        pub iface: *mut vpx_codec_iface_t,
        pub err: vpx_codec_err_t,
        pub err_detail: *const c_char,
        pub init_flags: c_long,
        pub config: *const c_void,
        pub priv_: *mut c_void,
    }

    #[repr(C)]
    pub struct vpx_image_t {
        pub fmt: c_int,
        pub cs: c_int,
        pub range: c_int,
        pub w: c_uint,
        pub h: c_uint,
        pub bit_depth: c_uint,
        pub d_w: c_uint,
        pub d_h: c_uint,
        pub r_w: c_uint,
        pub r_h: c_uint,
        pub x_chroma_shift: c_uint,
        pub y_chroma_shift: c_uint,
        pub planes: [*mut u8; 4],
        pub stride: [c_int; 4],
        pub bps: c_int,
        pub user_priv: *mut c_void,
        pub img_data: *mut u8,
        pub img_data_owner: c_int,
        pub self_allocd: c_int,
        pub fb_priv: *mut c_void,
    }

    unsafe extern "C" {
        pub fn vpx_codec_vp9_dx() -> *mut vpx_codec_iface_t;
        pub fn vpx_codec_dec_init_ver(
            ctx: *mut vpx_codec_ctx_t,
            iface: *mut vpx_codec_iface_t,
            cfg: *const vpx_codec_dec_cfg_t,
            flags: c_long,
            ver: c_int,
        ) -> vpx_codec_err_t;
        pub fn vpx_codec_decode(
            ctx: *mut vpx_codec_ctx_t,
            data: *const u8,
            data_sz: c_uint,
            user_priv: *mut c_void,
            deadline: c_long,
        ) -> vpx_codec_err_t;
        pub fn vpx_codec_get_frame(
            ctx: *mut vpx_codec_ctx_t,
            iter: *mut *const c_void,
        ) -> *mut vpx_image_t;
        pub fn vpx_codec_control_(
            ctx: *mut vpx_codec_ctx_t,
            ctrl_id: c_int,
            ...
        ) -> vpx_codec_err_t;
        pub fn vpx_codec_destroy(ctx: *mut vpx_codec_ctx_t) -> vpx_codec_err_t;
        pub fn vpx_codec_err_to_string(err: vpx_codec_err_t) -> *const c_char;
    }
}

/// Matrix coefficients signalled by the VP9 bitstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpace {
    Unknown,
    Bt601,
    Bt709,
    Smpte170,
    Smpte240,
    Bt2020,
    /// RGB stored as G, B, R in the Y, U, V planes.
    Srgb,
}

/// A libvpx VP9 decoder instance.
pub struct Decoder {
    // libvpx keeps internal pointers to the context, so it must not move.
    ctx: Box<ffi::vpx_codec_ctx_t>,
}

impl Decoder {
    /// Creates a decoder that uses up to `threads` worker threads.
    ///
    /// # Errors
    ///
    /// Returns an error when libvpx cannot initialize the decoder.
    pub fn new(threads: u32) -> Result<Self> {
        let mut ctx = Box::new(ffi::vpx_codec_ctx_t {
            name: ptr::null(),
            iface: ptr::null_mut(),
            err: ffi::VPX_CODEC_OK,
            err_detail: ptr::null(),
            init_flags: 0,
            config: ptr::null(),
            priv_: ptr::null_mut(),
        });
        let cfg = ffi::vpx_codec_dec_cfg_t {
            threads: threads.max(1),
            w: 0,
            h: 0,
        };
        // SAFETY: `ctx` and `cfg` are valid for the call; libvpx copies `cfg`.
        let status = unsafe {
            ffi::vpx_codec_dec_init_ver(
                &raw mut *ctx,
                ffi::vpx_codec_vp9_dx(),
                &raw const cfg,
                0,
                ffi::VPX_DECODER_ABI_VERSION,
            )
        };
        if status != ffi::VPX_CODEC_OK {
            bail!("cannot initialize VP9 decoder: {}", describe(status, &ctx));
        }
        let mut decoder = Self { ctx };
        if threads > 1 {
            // Row-based multithreading also parallelizes single-tile streams.
            // SAFETY: VP9D_SET_ROW_MT takes one int argument.
            let status = unsafe {
                ffi::vpx_codec_control_(
                    &raw mut *decoder.ctx,
                    ffi::VP9D_SET_ROW_MT,
                    c_int::from(true),
                )
            };
            decoder.check(status, "cannot enable VP9 row multithreading")?;
        }
        Ok(decoder)
    }

    /// Decodes one VP9 packet, including superframes, and returns the frame it
    /// shows, if any.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt or unsupported bitstreams, including
    /// high-bit-depth profiles.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<Image<'_>>> {
        let size = c_uint::try_from(data.len())?;
        // SAFETY: `data` outlives the call and libvpx does not keep the pointer.
        let status = unsafe {
            ffi::vpx_codec_decode(&raw mut *self.ctx, data.as_ptr(), size, ptr::null_mut(), 0)
        };
        self.check(status, "cannot decode VP9 frame")?;
        let mut iter: *const c_void = ptr::null();
        // SAFETY: the context is initialized; the returned image stays valid
        // until the next decode call, which the borrow of `self` prevents.
        let image = unsafe { ffi::vpx_codec_get_frame(&raw mut *self.ctx, &raw mut iter) };
        // SAFETY: libvpx returns null or a valid image.
        let Some(image) = (unsafe { image.as_ref() }) else {
            return Ok(None);
        };
        ensure!(
            image.fmt & ffi::VPX_IMG_FMT_HIGHBITDEPTH == 0,
            "high-bit-depth VP9 is unsupported"
        );
        Ok(Some(Image {
            raw: image,
            _decoder: PhantomData,
        }))
    }

    fn check(&self, status: ffi::vpx_codec_err_t, context: &str) -> Result<()> {
        if status == ffi::VPX_CODEC_OK {
            Ok(())
        } else {
            bail!("{context}: {}", describe(status, &self.ctx))
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the context was initialized in `new` and is destroyed once.
        unsafe { ffi::vpx_codec_destroy(&raw mut *self.ctx) };
    }
}

fn describe(status: ffi::vpx_codec_err_t, ctx: &ffi::vpx_codec_ctx_t) -> String {
    let text = |pointer: *const c_char| {
        // SAFETY: libvpx returns static or context-owned C strings.
        (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_string_lossy())
    };
    // SAFETY: vpx_codec_err_to_string accepts any status value.
    let message = text(unsafe { ffi::vpx_codec_err_to_string(status) }).unwrap_or_default();
    match text(ctx.err_detail) {
        Some(detail) => format!("{message} ({detail})"),
        None => message.into_owned(),
    }
}

/// An 8-bit decoded picture borrowed from its decoder.
pub struct Image<'a> {
    raw: &'a ffi::vpx_image_t,
    _decoder: PhantomData<&'a mut Decoder>,
}

impl Image<'_> {
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.raw.d_w
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.raw.d_h
    }

    /// Whether chroma is halved horizontally and vertically.
    #[must_use]
    pub const fn subsampling(&self) -> (bool, bool) {
        (self.raw.x_chroma_shift != 0, self.raw.y_chroma_shift != 0)
    }

    #[must_use]
    pub const fn color_space(&self) -> ColorSpace {
        match self.raw.cs {
            1 => ColorSpace::Bt601,
            2 => ColorSpace::Bt709,
            3 => ColorSpace::Smpte170,
            4 => ColorSpace::Smpte240,
            5 => ColorSpace::Bt2020,
            7 => ColorSpace::Srgb,
            _ => ColorSpace::Unknown,
        }
    }

    #[must_use]
    pub const fn full_range(&self) -> bool {
        self.raw.range == 1
    }

    /// Row stride in bytes of plane 0 (Y), 1 (U), or 2 (V).
    #[must_use]
    pub const fn stride(&self, plane: usize) -> u32 {
        self.raw.stride[plane].unsigned_abs()
    }

    /// Samples of plane 0 (Y), 1 (U), or 2 (V), from the first visible
    /// sample through the last visible sample of the last row.
    #[must_use]
    pub fn plane(&self, plane: usize) -> &[u8] {
        let (width, height) = if plane == 0 {
            (self.raw.d_w, self.raw.d_h)
        } else {
            let shift = |size: c_uint, by: c_uint| (size + (1 << by) - 1) >> by;
            (
                shift(self.raw.d_w, self.raw.x_chroma_shift),
                shift(self.raw.d_h, self.raw.y_chroma_shift),
            )
        };
        let stride = self.stride(plane) as usize;
        let len = stride * (height as usize).saturating_sub(1) + width as usize;
        // SAFETY: libvpx planes hold `height` rows of `stride` bytes each, and
        // VP9 output always uses positive strides.
        unsafe { std::slice::from_raw_parts(self.raw.planes[plane], len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_initializes_and_rejects_garbage() {
        let mut decoder = Decoder::new(2).unwrap();
        assert!(decoder.decode(&[0xff; 16]).is_err());
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn image_layout_matches_c_headers() {
        // Offsets printed by a C program built against libvpx v1.17.0 headers.
        assert_eq!(std::mem::size_of::<ffi::vpx_image_t>(), 136);
        assert_eq!(std::mem::offset_of!(ffi::vpx_image_t, planes), 48);
        assert_eq!(std::mem::offset_of!(ffi::vpx_image_t, stride), 80);
        assert_eq!(std::mem::offset_of!(ffi::vpx_image_t, fb_priv), 128);
        assert_eq!(std::mem::offset_of!(ffi::vpx_codec_ctx_t, priv_), 48);
    }
}
