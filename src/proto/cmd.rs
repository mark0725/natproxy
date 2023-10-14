use serde::{Deserialize, Serialize};
use crate::MappingConfig;
use crate::{
    get_datetime14,
    generate_uuid,
};

lazy_static! {
    static ref PROTO_CMD:Vec<u8> = vec![0x18u8, 0x11u8];
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProtoCmdBody {
    ClientConfData {
        mappings: Vec<MappingConfig>,
    },

    ProxyRequest {
        bind_id: String,
        client: String,
        mapping: MappingConfig,    
    },

    ProxyResponse {
        bind_id: String,
        client: String,
        mapping_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoCmdRequest {
    pub id: String,
    pub cmd_type: String,
    pub body: Option<ProtoCmdBody>,
    pub time: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoCmdResponse {
    pub id: String,
    pub cmd_type: String,
    pub status: String,
    pub message: String,
    pub body: Option<ProtoCmdBody>,
    pub time: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProtoCmd {
    Request(ProtoCmdRequest),
    Response(ProtoCmdResponse),
}

impl ProtoCmdRequest {
    pub fn new(cmd_type: String, body: Option<ProtoCmdBody>) -> Self {
        Self { 
            id: generate_uuid(),
            cmd_type, 
            time: get_datetime14(),
            body,
        }
    }

}

impl ProtoCmdResponse {
    pub fn new(id: String, cmd_type: String, status: String, message: String, body: Option<ProtoCmdBody>) -> Self {
        Self { 
            /// 原消息ID
            id,  
            /// 原消息类型
            cmd_type,
            /// 请求处理结果状态
            status, 
            /// 请求处理信息
            message, 
            time: get_datetime14(),
            body,
        }
    }

}
