use super::dev::*;

#[derive(Default)]
pub struct MockCommandUtil {}

impl<U: UserImpl + Send + Sync> From<MockCommandUtil> for BoxedCommandUtil<U> {
    fn from(value: MockCommandUtil) -> Self {
        Box::new(value)
    }
}

#[async_trait::async_trait]
impl<U: UserImpl + Send + Sync> CommandUtil<U> for MockCommandUtil {}
