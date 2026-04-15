use spvirit_codec::spvd_encode::encode_pv_request;

/// Build a pvRequest structure for "field(value,alarm,timeStamp)".
///
/// This is the standard pvRequest used by GET and MONITOR operations
/// to request the value plus alarm and timestamp metadata.
pub fn build_pv_request(big_endian: bool) -> Vec<u8> {
    encode_pv_request(&["value", "alarm", "timeStamp"], big_endian)
}

/// Build a minimal pvRequest for PUT: "field(value)".
pub fn build_pv_request_value_only(big_endian: bool) -> Vec<u8> {
    encode_pv_request(&["value"], big_endian)
}
