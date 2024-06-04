use anyhow::Context;
use authentication::application_auth::ApplicationAuth;
use common::types::MemberId;
use errors::ErrorMetadata;
use keybroker::{
    AdminIdentityPrincipal,
    Identity,
    KeyBroker,
};

pub fn must_be_admin_from_keybroker(
    kb: &KeyBroker,
    instance_name: Option<String>,
    admin_key: String,
) -> anyhow::Result<Identity> {
    let identity = kb
        .check_admin_key(&admin_key)
        .context(bad_admin_key_error(instance_name))?;
    Ok(identity)
}

pub async fn must_be_admin_from_key(
    app_auth: &ApplicationAuth,
    instance_name: String,
    admin_key_or_access_token: String,
) -> anyhow::Result<Identity> {
    let identity = app_auth
        .check_key(admin_key_or_access_token, instance_name.clone())
        .await
        .context(bad_admin_key_error(Some(instance_name)))?;
    Ok(identity)
}

pub fn must_be_admin(identity: &Identity) -> anyhow::Result<AdminIdentityPrincipal> {
    if let Identity::InstanceAdmin(admin_identity) = identity {
        Ok(admin_identity.principal().clone())
    } else {
        Err(bad_admin_key_error(identity.instance_name()).into())
    }
}

pub fn must_be_admin_member(identity: &Identity) -> anyhow::Result<MemberId> {
    if let Identity::InstanceAdmin(admin_identity) = identity {
        if let AdminIdentityPrincipal::Member(member_id) = admin_identity.principal() {
            Ok(*member_id)
        } else {
            Err(bad_admin_key_error(identity.instance_name()).into())
        }
    } else {
        Err(bad_admin_key_error(identity.instance_name()).into())
    }
}

pub fn bad_admin_key_error(instance_name: Option<String>) -> ErrorMetadata {
    let msg = match instance_name {
        Some(name) => format!(
            "The provided deploy key was invalid for deployment '{}'. Double check that the \
             environment this key was generated for matches the desired deployment.",
            name
        ),
        None => "The provided deploy key was invalid for this deployment. Double check that the \
                 environment this key was generated for matches the desired deployment."
            .to_string(),
    };
    ErrorMetadata::forbidden("BadDeployKey", msg)
}
