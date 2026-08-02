// Shared layout notes only. The implementation should generate or validate host/shader layouts.

struct RangeParams {
    source_word_offset: u32,
    source_byte_len: u32,
    output_offset: u32,
    flags: u32,
};
