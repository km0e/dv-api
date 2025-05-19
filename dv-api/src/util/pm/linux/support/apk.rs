use super::dev::*;

#[derive(Default)]
pub struct Apk {}

#[async_trait::async_trait]
impl Am for Apk {
    async fn install(
        &self,
        u: &User,
        interactor: &DynInteractor,
        packages: &str,
    ) -> crate::Result<bool> {
        super::install(
            u,
            interactor,
            format!("pkgs=\"{}\";", packages),
            include_str!("sh/apk_query.sh"),
            "apk",
            &["add"][..],
        )
        .await
    }
}
