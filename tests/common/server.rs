use std::net::SocketAddr;
use std::sync::Arc;

use akasa_core::{
    mqtt_proto,
    server::{handle_accept, ConnectionArgs},
    Config, GlobalState, Hook, HookAction, HookConnectCode, HookPublishCode, HookResult,
    HookSubscribeCode, HookUnsubscribeCode, Listener, SessionV3, SessionV5,
};
use tokio::net::TcpListener;

pub async fn server() -> SocketAddr {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut config = Config::new_allow_anonymous();
    config.listeners.mqtt = Some(Listener {
        addr,
        reuse_port: false,
        proxy_mode: None,
    });

    let global = Arc::new(GlobalState::new(config));

    tokio::spawn(async move {
        let conn_args = ConnectionArgs {
            addr,
            reuse_port: false,
            proxy: false,
            proxy_tls_termination: false,
            websocket: false,
            tls_acceptor: None,
        };
        loop {
            let Ok((conn, peer)) = listener.accept().await else {
                break;
            };
            let global = Arc::clone(&global);
            let conn_args = conn_args.clone();
            tokio::spawn(handle_accept(conn, conn_args, peer, NoopHook, global));
        }
    });

    addr
}

#[derive(Clone)]
pub struct NoopHook;

impl Hook for NoopHook {
    async fn v3_before_connect(
        &self,
        _peer: SocketAddr,
        _connect: &mqtt_proto::v3::Connect,
    ) -> HookResult<HookConnectCode> {
        Ok(HookConnectCode::Success)
    }

    async fn v3_after_connect(
        &self,
        _session: &SessionV3,
        _session_present: bool,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v3_before_publish(
        &self,
        _session: &SessionV3,
        _encode_len: usize,
        _packet_body: &[u8],
        _publish: &mut mqtt_proto::v3::Publish,
        _changed: &mut bool,
    ) -> HookResult<HookPublishCode> {
        Ok(HookPublishCode::Success)
    }

    async fn v3_after_publish(
        &self,
        _session: &SessionV3,
        _encode_len: usize,
        _packet_body: &[u8],
        _publish: &mqtt_proto::v3::Publish,
        _changed: bool,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v3_before_subscribe(
        &self,
        _session: &SessionV3,
        _encode_len: usize,
        _packet_body: &[u8],
        _subscribe: &mut mqtt_proto::v3::Subscribe,
        _changed: &mut bool,
    ) -> HookResult<HookSubscribeCode> {
        Ok(HookSubscribeCode::Success)
    }

    async fn v3_after_subscribe(
        &self,
        _session: &SessionV3,
        _encode_len: usize,
        _packet_body: &[u8],
        _subscribe: &mqtt_proto::v3::Subscribe,
        _changed: bool,
        _codes: Option<Vec<mqtt_proto::v3::SubscribeReturnCode>>,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v3_before_unsubscribe(
        &self,
        _session: &SessionV3,
        _encode_len: usize,
        _packet_body: &[u8],
        _unsubscribe: &mut mqtt_proto::v3::Unsubscribe,
        _changed: &mut bool,
    ) -> HookResult<HookUnsubscribeCode> {
        Ok(HookUnsubscribeCode::Success)
    }

    async fn v3_after_unsubscribe(
        &self,
        _session: &SessionV3,
        _encode_len: usize,
        _packet_body: &[u8],
        _unsubscribe: &mqtt_proto::v3::Unsubscribe,
        _changed: bool,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v3_after_disconnect(&self, _session: &SessionV3, _taken_over: bool) -> HookResult<()> {
        Ok(())
    }

    async fn v5_before_connect(
        &self,
        _peer: SocketAddr,
        _connect: &mqtt_proto::v5::Connect,
    ) -> HookResult<HookConnectCode> {
        Ok(HookConnectCode::Success)
    }

    async fn v5_after_connect(
        &self,
        _session: &SessionV5,
        _session_present: bool,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v5_before_publish(
        &self,
        _session: &SessionV5,
        _encode_len: usize,
        _packet_body: &[u8],
        _publish: &mut mqtt_proto::v5::Publish,
        _changed: &mut bool,
    ) -> HookResult<HookPublishCode> {
        Ok(HookPublishCode::Success)
    }

    async fn v5_after_publish(
        &self,
        _session: &SessionV5,
        _encode_len: usize,
        _packet_body: &[u8],
        _publish: &mqtt_proto::v5::Publish,
        _changed: bool,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v5_before_subscribe(
        &self,
        _session: &SessionV5,
        _encode_len: usize,
        _packet_body: &[u8],
        _subscribe: &mut mqtt_proto::v5::Subscribe,
        _changed: &mut bool,
    ) -> HookResult<HookSubscribeCode> {
        Ok(HookSubscribeCode::Success)
    }

    async fn v5_after_subscribe(
        &self,
        _session: &SessionV5,
        _encode_len: usize,
        _packet_body: &[u8],
        _subscribe: &mqtt_proto::v5::Subscribe,
        _changed: bool,
        _codes: Option<Vec<mqtt_proto::v5::SubscribeReasonCode>>,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v5_before_unsubscribe(
        &self,
        _session: &SessionV5,
        _encode_len: usize,
        _packet_body: &[u8],
        _unsubscribe: &mut mqtt_proto::v5::Unsubscribe,
        _changed: &mut bool,
    ) -> HookResult<HookUnsubscribeCode> {
        Ok(HookUnsubscribeCode::Success)
    }

    async fn v5_after_unsubscribe(
        &self,
        _session: &SessionV5,
        _encode_len: usize,
        _packet_body: &[u8],
        _unsubscribe: &mqtt_proto::v5::Unsubscribe,
        _changed: bool,
    ) -> HookResult<Vec<HookAction>> {
        Ok(vec![])
    }

    async fn v5_after_disconnect(&self, _session: &SessionV5, _taken_over: bool) -> HookResult<()> {
        Ok(())
    }
}
