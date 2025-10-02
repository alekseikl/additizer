pub const BUFFER_SIZE: usize = 128;
pub type Buffer = [f32; BUFFER_SIZE];
pub const ZEROES_BUFFER: Buffer = [0.0; BUFFER_SIZE];
pub const ONES_BUFFER: Buffer = [1.0; BUFFER_SIZE];

#[inline]
pub fn make_zero_buffer() -> Buffer {
    [0.0; BUFFER_SIZE]
}
