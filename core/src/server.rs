use crate::dal::PanoramaDatabase;
use crate::server_proto::panorama_server::Panorama as PanoramaServerProto;

pub struct Panorama {
  db: PanoramaDatabase,
}

impl Panorama {}

#[tonic::async_trait]
impl PanoramaServerProto for Panorama {}
