use tonic::{Request, Response, Result};

use crate::dal::PanoramaDatabase;
use crate::server_proto::panorama_server::Panorama as PanoramaServerProto;
use crate::server_proto::{EnsureMailAccountReply, EnsureMailAccountRequest};

pub struct Panorama {
  db: PanoramaDatabase,
}

impl Panorama {}

#[tonic::async_trait]
impl PanoramaServerProto for Panorama {
  async fn ensure_mail_account(
    &self,
    req: Request<EnsureMailAccountRequest>,
  ) -> Result<Response<EnsureMailAccountReply>> {
    todo!()
  }
}
