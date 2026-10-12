use anyhow::{Context, Result, ensure};

/// An owned 8-bit plane, including the decoder's row padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plane {
    pub data: Vec<u8>,
    pub stride: u32,
    pub width: u32,
    pub height: u32,
}

impl Plane {
    /// Copies visible rows while retaining their stride. The last row need not
    /// include padding, as with decoder buffers and GPU texture uploads.
    ///
    /// # Errors
    /// Returns an error for an invalid stride or a truncated plane.
    pub fn copy(data: &[u8], stride: u32, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && stride >= width,
            "invalid video plane dimensions"
        );
        let length = usize::try_from(u64::from(height - 1) * u64::from(stride) + u64::from(width))?;
        let data = data
            .get(..length)
            .context("truncated video plane")?
            .to_vec();
        Ok(Self {
            data,
            stride,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMatrix {
    Bt601,
    Bt709,
    Bt2020,
    Smpte240,
    /// VP9 RGB mode stores G, B, R in the Y, U, V planes.
    Identity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorRange {
    Limited,
    Full,
}

/// Native planar video samples. RGB remains straight when alpha is present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub planes: [Plane; 3],
    pub alpha: Option<Plane>,
    pub matrix: ColorMatrix,
    pub range: ColorRange,
    pub subsampling: (bool, bool),
}

impl VideoFrame {
    /// # Errors
    /// Returns an error for truncated or inconsistent picture planes.
    pub fn copy(
        (width, height): (u32, u32),
        planes: [(&[u8], u32); 3],
        subsampling: (bool, bool),
        range: ColorRange,
        matrix: ColorMatrix,
    ) -> Result<Self> {
        let chroma_width = if subsampling.0 {
            width.div_ceil(2)
        } else {
            width
        };
        let chroma_height = if subsampling.1 {
            height.div_ceil(2)
        } else {
            height
        };
        let [y, u, v] = planes;
        Ok(Self {
            planes: [
                Plane::copy(y.0, y.1, width, height)?,
                Plane::copy(u.0, u.1, chroma_width, chroma_height)?,
                Plane::copy(v.0, v.1, chroma_width, chroma_height)?,
            ],
            alpha: None,
            matrix,
            range,
            subsampling,
        })
    }

    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.planes[0].width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.planes[0].height
    }

    /// Rows of the affine YUV-to-RGB transform, applied to normalized samples.
    #[must_use]
    pub fn rgb_transform(&self) -> [[f32; 4]; 3] {
        let (kr, kb) = match self.matrix {
            ColorMatrix::Bt601 => (0.299, 0.114),
            ColorMatrix::Bt709 => (0.2126, 0.0722),
            ColorMatrix::Bt2020 => (0.2627, 0.0593),
            ColorMatrix::Smpte240 => (0.212, 0.087),
            ColorMatrix::Identity => {
                return [
                    [0.0, 0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                ];
            }
        };
        let kg = 1.0 - kr - kb;
        let (ys, cs, offset) = match self.range {
            ColorRange::Limited => (255.0 / 219.0, 255.0 / 224.0, 16.0 / 219.0),
            ColorRange::Full => (1.0, 1.0, 0.0),
        };
        let rv = 2.0 * (1.0 - kr) * cs;
        let bu = 2.0 * (1.0 - kb) * cs;
        let gu = -2.0 * kb * (1.0 - kb) / kg * cs;
        let gv = -2.0 * kr * (1.0 - kr) / kg * cs;
        let center = 128.0 / 255.0;
        [
            [ys, 0.0, rv, f32::mul_add(rv, -center, -offset)],
            [ys, gu, gv, f32::mul_add(gu + gv, -center, -offset)],
            [ys, bu, 0.0, f32::mul_add(bu, -center, -offset)],
        ]
    }
}
