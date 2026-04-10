/// Codec names for compressed NDArray data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecName {
    None,
    JPEG,
    LZ4,
    Blosc,
    BSLZ4,
}

/// Codec information attached to an NDArray (matching C++ Codec_t).
#[derive(Debug, Clone)]
pub struct Codec {
    pub name: CodecName,
    pub compressed_size: usize,
    pub level: i32,
    pub shuffle: i32,
    pub compressor: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_clone() {
        let c = Codec {
            name: CodecName::LZ4,
            compressed_size: 1024,
            level: 0,
            shuffle: 0,
            compressor: 0,
        };
        let c2 = c.clone();
        assert_eq!(c2.name, CodecName::LZ4);
        assert_eq!(c2.compressed_size, 1024);
    }

    #[test]
    fn test_codec_name_none() {
        assert_eq!(CodecName::None, CodecName::None);
        assert_ne!(CodecName::None, CodecName::JPEG);
    }
}
