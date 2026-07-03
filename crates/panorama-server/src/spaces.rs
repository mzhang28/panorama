use std::sync::Arc;


/// Simple space model for v0.0.
/// Spaces handle multi-user permissions at the space level.
#[derive(Debug, Clone)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub owner_user_id: String,
    pub members: Vec<SpaceMember>,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct SpaceMember {
    pub user_id: String,
    pub role: SpaceRole,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpaceRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

/// Simple space manager.
/// For v0.0, this is mostly a stub - real auth will come later.
#[derive(Clone, Default)]
pub struct SpaceManager {
    spaces: Arc<dashmap::DashMap<String, Space>>,
}

impl SpaceManager {
    pub fn new() -> Self {
        Self {
            spaces: Arc::new(dashmap::DashMap::new()),
        }
    }

    pub fn create(&self, name: &str, owner_user_id: &str) -> Space {
        let id = format!("space_{}", uuid::Uuid::new_v4());
        let space = Space {
            id: id.clone(),
            name: name.to_string(),
            owner_user_id: owner_user_id.to_string(),
            members: vec![SpaceMember {
                user_id: owner_user_id.to_string(),
                role: SpaceRole::Owner,
            }],
            is_public: false,
        };
        self.spaces.insert(id, space.clone());
        space
    }

    pub fn get(&self, id: &str) -> Option<Space> {
        self.spaces.get(id).map(|s| s.clone())
    }

    pub fn list_for_user(&self, user_id: &str) -> Vec<Space> {
        self.spaces
            .iter()
            .filter(|s| s.members.iter().any(|m| m.user_id == user_id))
            .map(|s| s.value().clone())
            .collect()
    }
}
