use super::dev::*;

pub async fn install(u: &User, interactor: &DynInteractor, packages: &str) -> Result<bool> {
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
