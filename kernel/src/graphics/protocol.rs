// ============ src/graphics/protocol.rs ============
#[repr(u32)]
pub enum GraphicsCommand {
    CreateSurface = 1,
    DestroySurface = 2,
    UpdateSurface = 3,
    BlitSurface = 4,
    PresentSurface = 5,
    SetPosition = 6,
    SetZOrder = 7,
}

#[repr(C)]
pub struct GraphicsRequest {
    pub command: u32,
    pub surface_id: u64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub data_len: usize,
}

#[repr(C)]
pub struct GraphicsResponse {
    pub success: bool,
    pub surface_id: u64,
    pub error_code: i32,
}

