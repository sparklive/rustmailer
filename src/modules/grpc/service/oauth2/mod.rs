use std::sync::Arc;

use crate::modules::common::auth::ClientContext;
use crate::modules::error::code::ErrorCode;
use crate::modules::grpc::service::rustmailer_grpc::{
    AuthorizeUrlRequest, AuthorizeUrlResponse, DeleteOAuth2Request, Empty, GetOAuth2Request,
    GetOAuth2TokensRequest, ListOAuth2Request, OAuth2, OAuth2AccessToken, OAuth2CreateRequest,
    OAuth2Service, PagedOAuth2, UpdateOAuth2Request,
};
use crate::modules::oauth2::{
    entity::OAuth2 as RustMailerOAuth2, flow::OAuth2Flow,
    token::OAuth2AccessToken as RustMailerOAuth2AccessToken,
};
use crate::raise_error;
use poem_grpc::{Request, Response, Status};

pub mod from;

#[derive(Default)]
pub struct RustMailerOAuth2Service;

impl OAuth2Service for RustMailerOAuth2Service {
    async fn get_o_auth2_config(
        &self,
        request: Request<GetOAuth2Request>,
    ) -> Result<Response<OAuth2>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;
        let result = RustMailerOAuth2::get(req.id).await?.ok_or_else(|| {
            raise_error!(
                format!("oauth2 id={} not found", req.id),
                ErrorCode::ResourceNotFound
            )
        })?;
        Ok(Response::new(result.into()))
    }

    async fn remove_o_auth2_config(
        &self,
        request: Request<DeleteOAuth2Request>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;
        RustMailerOAuth2::delete(req.id).await?;
        Ok(Response::new(Empty::default()))
    }

    async fn create_o_auth2_config(
        &self,
        request: Request<OAuth2CreateRequest>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;
        let oauth2 = RustMailerOAuth2::new(req.into())?;
        oauth2.save().await?;
        Ok(Response::new(Empty::default()))
    }

    async fn update_o_auth2_config(
        &self,
        request: Request<UpdateOAuth2Request>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;
        RustMailerOAuth2::update(req.id, req.into()).await?;
        Ok(Response::new(Empty::default()))
    }

    async fn list_o_auth2_config(
        &self,
        request: Request<ListOAuth2Request>,
    ) -> Result<Response<PagedOAuth2>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;
        let result = RustMailerOAuth2::paginate_list(req.page, req.page_size, req.desc).await?;
        Ok(Response::new(result.into()))
    }

    async fn create_authorize_url(
        &self,
        request: Request<AuthorizeUrlRequest>,
    ) -> Result<Response<AuthorizeUrlResponse>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;
        let flow = OAuth2Flow::new(req.oauth2_id);
        let url = flow.authorize_url(req.account_id).await?;
        Ok(Response::new(AuthorizeUrlResponse { url }))
    }

    async fn get_o_auth2_tokens(
        &self,
        request: Request<GetOAuth2TokensRequest>,
    ) -> Result<Response<OAuth2AccessToken>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_account_access(req.account_id)?;
        let result = RustMailerOAuth2AccessToken::get(req.account_id)
            .await?
            .ok_or_else(|| {
                raise_error!(
                    "oauth2 access token not found".into(),
                    ErrorCode::ResourceNotFound
                )
            })?;
        Ok(Response::new(result.into()))
    }
}
