//! Indirect draw args (MDI): el compute shader genera los draws,
//! la CPU no hace loop por chunk. Cero read-back.

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IndirectDrawArgs {
    pub vertex_count: u32,
    pub instance_count: u32, // 0 = culled, 1 = visible
    pub first_vertex: u32,
    pub first_instance: u32,
}
