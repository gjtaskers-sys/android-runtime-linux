//! Stable host-display wire format.
//!
//! Android-side HWC code sends one final, already-composited client target to
//! the host. The actual DMA-BUF file descriptor is transferred separately via
//! SCM_RIGHTS; this header contains only metadata.

pub const MAGIC: [u8; 8] = *b"ANDRFRM1";
pub const VERSION: u32 = 4;
pub const ACK_MAGIC: [u8; 8] = *b"ANDRACK1";

/// Defensive bounds applied before mapping an incoming frame buffer.
pub const MAX_FRAME_DIMENSION: u32 = 16_384;
pub const MAX_FRAME_BYTES: u64 = 256 * 1024 * 1024;

pub const FLAG_CLIENT_TARGET: u32 = 1 << 0;
pub const FLAG_DMA_BUF_CPU_SYNC: u32 = 1 << 1;
pub const KNOWN_FRAME_FLAGS: u32 = FLAG_CLIENT_TARGET | FLAG_DMA_BUF_CPU_SYNC;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub drm_fourcc: u32,
    pub modifier_lo: u32,
    pub modifier_hi: u32,
    pub flags: u32,
    pub sequence: u64,
}

impl FrameHeader {
    pub fn new(width: u32, height: u32, stride: u32, fourcc: u32, sequence: u64) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            width,
            height,
            stride,
            drm_fourcc: fourcc,
            modifier_lo: 0,
            modifier_hi: 0,
            flags: 0,
            sequence,
        }
    }

    /// Validate the wire header and memory bounds before a receiver maps a buffer.
    /// Pixel format and modifiers are additionally checked by the selected display backend.
    pub fn is_valid(&self) -> bool {
        if self.magic != MAGIC
            || self.version != VERSION
            || self.width == 0
            || self.height == 0
            || self.width > MAX_FRAME_DIMENSION
            || self.height > MAX_FRAME_DIMENSION
            || self.flags & !KNOWN_FRAME_FLAGS != 0
        {
            return false;
        }

        let Some(min_stride) = self.width.checked_mul(4) else {
            return false;
        };
        if self.stride < min_stride {
            return false;
        }

        u64::from(self.stride)
            .checked_mul(u64::from(self.height))
            .is_some_and(|bytes| bytes <= MAX_FRAME_BYTES)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameAck {
    pub magic: [u8; 8],
    pub version: u32,
    pub status: u32,
    pub sequence: u64,
}

impl FrameAck {
    pub fn is_valid_for(&self, sequence: u64) -> bool {
        self.magic == ACK_MAGIC
            && self.version == VERSION
            && self.status == 0
            && self.sequence == sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258;

    #[test]
    fn wire_struct_sizes_match_c_layout() {
        assert_eq!(size_of::<FrameHeader>(), 48);
        assert_eq!(size_of::<FrameAck>(), 24);
    }

    #[test]
    fn accepts_a_normal_720p_frame() {
        let mut header = FrameHeader::new(1280, 720, 1280 * 4, DRM_FORMAT_XRGB8888, 7);
        header.flags = FLAG_CLIENT_TARGET;
        assert!(header.is_valid());
    }

    #[test]
    fn accepts_android_dma_buf_frame_requiring_cpu_sync() {
        let mut header = FrameHeader::new(1280, 720, 1280 * 4, DRM_FORMAT_XRGB8888, 8);
        header.flags = FLAG_CLIENT_TARGET | FLAG_DMA_BUF_CPU_SYNC;
        assert!(header.is_valid());
    }

    #[test]
    fn rejects_zero_or_excessive_dimensions() {
        assert!(!FrameHeader::new(0, 720, 0, DRM_FORMAT_XRGB8888, 1).is_valid());
        assert!(!FrameHeader::new(1280, 0, 1280 * 4, DRM_FORMAT_XRGB8888, 1).is_valid());
        assert!(!FrameHeader::new(
            MAX_FRAME_DIMENSION + 1,
            1,
            (MAX_FRAME_DIMENSION + 1) * 4,
            DRM_FORMAT_XRGB8888,
            1,
        )
        .is_valid());
    }

    #[test]
    fn rejects_stride_smaller_than_a_pixel_row() {
        assert!(!FrameHeader::new(1280, 720, 1280 * 4 - 1, DRM_FORMAT_XRGB8888, 1).is_valid());
    }

    #[test]
    fn rejects_frame_memory_over_the_safety_limit() {
        assert!(!FrameHeader::new(
            8193,
            8192,
            8193 * 4,
            DRM_FORMAT_XRGB8888,
            1,
        )
        .is_valid());
    }

    #[test]
    fn rejects_unknown_frame_flags() {
        let mut header = FrameHeader::new(1280, 720, 1280 * 4, DRM_FORMAT_XRGB8888, 1);
        header.flags = 1 << 31;
        assert!(!header.is_valid());
    }

    #[test]
    fn acknowledgement_must_match_magic_version_status_and_sequence() {
        let valid = FrameAck {
            magic: ACK_MAGIC,
            version: VERSION,
            status: 0,
            sequence: 42,
        };
        assert!(valid.is_valid_for(42));
        assert!(!valid.is_valid_for(43));
        assert!(!FrameAck { status: 1, ..valid }.is_valid_for(42));
        assert!(!FrameAck { version: VERSION + 1, ..valid }.is_valid_for(42));
        assert!(!FrameAck { magic: [0; 8], ..valid }.is_valid_for(42));
    }
}
