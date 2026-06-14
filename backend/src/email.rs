use anyhow::{Context, Result};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, SinglePart, header::ContentType},
};

use crate::config::Config;

#[derive(Clone)]
pub struct EmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    app_base_url: String,
}

impl EmailSender {
    pub fn new(config: &Config) -> Result<Self> {
        let from = config
            .smtp_from
            .parse()
            .context("SMTP_FROM must be a valid mailbox")?;
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
            .port(config.smtp_port)
            .build();

        Ok(Self {
            transport,
            from,
            app_base_url: config.app_base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn send_password_reset(
        &self,
        recipient: &str,
        tenant_name: &str,
        token: &str,
    ) -> Result<()> {
        let reset_url = format!("{}/reset-password?token={token}", self.app_base_url);
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient
                .parse()
                .context("password reset recipient must be a valid mailbox")?)
            .subject("パスワード再設定のご案内")
            .singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(format!(
                        "{tenant_name} のパスワード再設定を受け付けました。\n\n\
                         次のURLから30分以内に新しいパスワードを設定してください。\n\
                         {reset_url}\n\n\
                         この操作に心当たりがない場合は、このメールを破棄してください。\n"
                    )),
            )
            .context("failed to build password reset email")?;

        self.transport
            .send(message)
            .await
            .context("failed to send password reset email")?;
        Ok(())
    }
}
