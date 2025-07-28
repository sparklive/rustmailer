// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::sync::Arc;

use crate::modules::common::auth::ClientContext;
use crate::modules::error::code::ErrorCode;
use crate::modules::grpc::service::rustmailer_grpc::SendTestEmailRequest;
use crate::modules::grpc::service::rustmailer_grpc::{
    DeleteMtaRequest, Empty, GetMtaRequest, ListMtaRequest, Mta, MtaCreateRequest, MtaService,
    MtaUpdateRequest, PagedMta,
};
use crate::modules::smtp::mta::entity::Mta as RustMailerMta;
use crate::modules::smtp::mta::send::send_test_email;
use crate::raise_error;
use poem_grpc::{Request, Response, Status};

pub mod from;

#[derive(Default)]
pub struct RustMailerMtaService;

impl MtaService for RustMailerMtaService {
    async fn get_mta(&self, request: Request<GetMtaRequest>) -> Result<Response<Mta>, Status> {
        let mta = RustMailerMta::get(request.into_inner().id)
            .await?
            .ok_or_else(|| raise_error!("mta not found".into(), ErrorCode::ResourceNotFound))?;
        Ok(Response::new(mta.into()))
    }

    async fn remove_mta(
        &self,
        request: Request<DeleteMtaRequest>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;

        RustMailerMta::delete(req.id).await?;
        Ok(Response::new(Empty::default()))
    }

    async fn create_mta(
        &self,
        request: Request<MtaCreateRequest>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;

        let entity = RustMailerMta::new(req.try_into().map_err(|e: &'static str| {
            raise_error!(e.to_string(), ErrorCode::InvalidParameter)
        })?)?;
        entity.save().await?;
        Ok(Response::new(Empty::default()))
    }

    async fn update_mta(
        &self,
        request: Request<MtaUpdateRequest>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;

        context.require_root()?;

        RustMailerMta::update(
            req.id,
            req.try_into().map_err(|e: &'static str| {
                raise_error!(e.to_string(), ErrorCode::InvalidParameter)
            })?,
        )
        .await?;
        Ok(Response::new(Empty::default()))
    }

    async fn list_mta(
        &self,
        request: Request<ListMtaRequest>,
    ) -> Result<Response<PagedMta>, Status> {
        let request = request.into_inner();
        let result =
            RustMailerMta::paginate_list(request.page, request.page_size, request.desc).await?;
        Ok(Response::new(result.into()))
    }

    async fn send_test_email(
        &self,
        request: Request<SendTestEmailRequest>,
    ) -> Result<Response<Empty>, Status> {
        let extensions = request.extensions().clone();
        let req = request.into_inner();

        // Get ClientContext from cloned extensions
        let context = extensions.get::<Arc<ClientContext>>().ok_or_else(|| {
            raise_error!("Missing ClientContext".into(), ErrorCode::InternalError)
        })?;
        context.require_root()?;

        send_test_email(req.mta_id, req.into()).await?;
        Ok(Response::new(Empty::default()))
    }
}
