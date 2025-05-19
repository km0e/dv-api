use super::dev::*;

pub async fn install(u: &User, interactor: &DynInteractor, packages: &str) -> Result<bool> {
    super::install(
        u,
        interactor,
        format!("am=yay;pkgs=\"{}\";", packages),
        include_str!("sh/pacman_query.sh"),
        "yay",
        &["-S", "--noconfirm"][..],
    )
    .await
}
