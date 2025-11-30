//src/reality/intent.rs
#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum Intent {
    NeedMemory = 0x0100,
    ReleaseMemory = 0x0101,
    ShareMemory = 0x0102,
    
    ReadData = 0x0200,
    WriteData = 0x0201,
    StreamData = 0x0202,
    
    SpawnTask = 0x0300,
    JoinTask = 0x0301,
    ForkReality = 0x0302,
    MergeReality = 0x0303,
    
    FindFile = 0x0400,
    StoreFile = 0x0401,
    OrganizeFiles = 0x0402,
    
    Checkpoint = 0x0500,
    Rollback = 0x0501,
    QueryCausality = 0x0502,
}

#[repr(C)]
pub struct IntentRequest {
    pub intent: Intent,
    pub priority: u8,
    pub context: [u8; 128],
    pub context_len: usize,
}

#[repr(C)]
pub struct IntentResponse {
    pub success: bool,
    pub result_code: i32,
    pub data: [u8; 256],
    pub data_len: usize,
}

impl IntentResponse {
    pub fn success(result_code: i32) -> Self {
        Self {
            success: true,
            result_code,
            data: [0; 256],
            data_len: 0,
        }
    }
    
    pub fn success_with_data(result_code: i32, data: &[u8]) -> Self {
        let mut response = Self::success(result_code);
        let len = data.len().min(256);
        response.data[..len].copy_from_slice(&data[..len]);
        response.data_len = len;
        response
    }
    
    pub fn error(result_code: i32) -> Self {
        Self {
            success: false,
            result_code,
            data: [0; 256],
            data_len: 0,
        }
    }
}

pub fn handle_intent(request: &IntentRequest) -> IntentResponse {
    match request.intent {
        Intent::NeedMemory => {
            IntentResponse::error(-1)
        }
        Intent::ReleaseMemory => {
            IntentResponse::success(0)
        }
        Intent::ShareMemory => {
            IntentResponse::error(-1)
        }
        Intent::ReadData => {
            IntentResponse::error(-1)
        }
        Intent::WriteData => {
            IntentResponse::error(-1)
        }
        Intent::StreamData => {
            IntentResponse::error(-1)
        }
        Intent::SpawnTask => {
            IntentResponse::error(-1)
        }
        Intent::JoinTask => {
            IntentResponse::error(-1)
        }
        Intent::ForkReality => {
            let new_reality = super::causality::RealityId::new();
            super::causality::set_current_reality(new_reality);
            IntentResponse::success(new_reality.as_u64() as i32)
        }
        Intent::MergeReality => {
            super::causality::set_current_reality(super::causality::RealityId::root());
            IntentResponse::success(0)
        }
        Intent::FindFile => {
            IntentResponse::error(-1)
        }
        Intent::StoreFile => {
            IntentResponse::error(-1)
        }
        Intent::OrganizeFiles => {
            IntentResponse::success(0)
        }
        Intent::Checkpoint => {
            IntentResponse::error(-1)
        }
        Intent::Rollback => {
            IntentResponse::error(-1)
        }
        Intent::QueryCausality => {
            let event_count = super::causality::get_total_events();
            IntentResponse::success(event_count as i32)
        }
    }
}