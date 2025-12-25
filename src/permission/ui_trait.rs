use super::types::{PermissionRequest, PermissionResponse};
use crate::core::error::Result;

pub trait PermissionUI: Send + Sync {
    fn prompt_user(&self, request: &PermissionRequest) -> Result<PermissionResponse>;
}

#[cfg(test)]
pub mod test_utils {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct HeadlessPermissionUI {
        response: PermissionResponse,
    }

    impl HeadlessPermissionUI {
        #[must_use]
        pub fn deny() -> Self {
            Self {
                response: PermissionResponse::TellModelDifferently(
                    "Permission denied (prompts disabled)".to_string(),
                ),
            }
        }

        #[must_use]
        pub const fn allow_once() -> Self {
            Self {
                response: PermissionResponse::AllowOnce,
            }
        }

        #[must_use]
        pub const fn allow_session() -> Self {
            Self {
                response: PermissionResponse::AllowSession,
            }
        }
    }

    impl PermissionUI for HeadlessPermissionUI {
        fn prompt_user(&self, _request: &PermissionRequest) -> Result<PermissionResponse> {
            Ok(self.response.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::HeadlessPermissionUI;
    use super::*;
    use crate::permission::types::PermissionType;

    #[test]
    fn test_headless_ui_deny() {
        let ui = HeadlessPermissionUI::deny();
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt");
        let response = ui.prompt_user(&request).unwrap();
        assert!(matches!(
            response,
            PermissionResponse::TellModelDifferently(_)
        ));
    }

    #[test]
    fn test_headless_ui_allow_once() {
        let ui = HeadlessPermissionUI::allow_once();
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt");
        let response = ui.prompt_user(&request).unwrap();
        assert!(matches!(response, PermissionResponse::AllowOnce));
    }

    #[test]
    fn test_headless_ui_allow_session() {
        let ui = HeadlessPermissionUI::allow_session();
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt");
        let response = ui.prompt_user(&request).unwrap();
        assert!(matches!(response, PermissionResponse::AllowSession));
    }
}
