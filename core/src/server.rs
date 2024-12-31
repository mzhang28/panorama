use tonic::{Request, Response, Result};
use sea_orm::DatabaseConnection;

use crate::dal::PanoramaDatabase;
use crate::server_proto::panorama_server::Panorama as PanoramaServerProto;
use crate::server_proto::{EnsureMailAccountReply, EnsureMailAccountRequest};

pub struct Panorama {
  db: PanoramaDatabase,
}

impl Panorama {
  pub async fn new(connection: DatabaseConnection) -> Self {
    Self {
      db: PanoramaDatabase::new(connection).await
    }
  }
}

#[tonic::async_trait]
impl PanoramaServerProto for Panorama {
  async fn ensure_mail_account(
    &self,
    req: Request<EnsureMailAccountRequest>,
  ) -> Result<Response<EnsureMailAccountReply>> {
    todo!()
  }
}
