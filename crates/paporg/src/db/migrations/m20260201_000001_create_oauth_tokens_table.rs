//! Migration to create the oauth_tokens table for storing OAuth2 tokens.

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create table
        manager
            .create_table(
                Table::create()
                    .table(OauthTokens::Table)
                    .if_not_exists()
                    .col(string(OauthTokens::SourceName).primary_key())
                    .col(string(OauthTokens::Provider).not_null())
                    .col(text(OauthTokens::AccessToken).not_null())
                    .col(text_null(OauthTokens::RefreshToken))
                    .col(timestamp_with_time_zone(OauthTokens::ExpiresAt).not_null())
                    .col(
                        timestamp_with_time_zone(OauthTokens::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(OauthTokens::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on Provider for filtering by provider
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_provider")
                    .table(OauthTokens::Table)
                    .col(OauthTokens::Provider)
                    .to_owned(),
            )
            .await?;

        // Create index on ExpiresAt for finding expired tokens
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_expires_at")
                    .table(OauthTokens::Table)
                    .col(OauthTokens::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        // Create composite index on (Provider, ExpiresAt) for efficient expired token queries per provider
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_provider_expires_at")
                    .table(OauthTokens::Table)
                    .col(OauthTokens::Provider)
                    .col(OauthTokens::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth_tokens_provider_expires_at")
                    .table(OauthTokens::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth_tokens_expires_at")
                    .table(OauthTokens::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth_tokens_provider")
                    .table(OauthTokens::Table)
                    .to_owned(),
            )
            .await?;

        // Drop table
        manager
            .drop_table(
                Table::drop()
                    .table(OauthTokens::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum OauthTokens {
    Table,
    SourceName,
    Provider,
    AccessToken,
    RefreshToken,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}
